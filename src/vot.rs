use crate::{
    config, servlet,
    shared::{self, VTo},
    ws::{self, WsRequest, WsResponse, WsResult},
};
use awc::ws::{Frame, Message};
use bytestring::ByteString;
use futures_util::{SinkExt as _, StreamExt as _};
use std::time::{Duration, Instant};
use tokio::{
    select,
    sync::mpsc::{self},
    time::{self},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

const INTERVAL: Duration = Duration::from_secs(3);
const START_AFTER: Duration = Duration::from_secs(3);
const LOCK_TIMEOUT: Duration = Duration::from_secs(9);

pub struct VoteHandler {
    addr: String,
    port: u16,
}

impl VoteHandler {
    pub fn new(addr: String, port: u16) -> Self {
        VoteHandler { addr, port }
    }

    // 启动
    pub async fn start(&self, server_id: usize, app_key: &str, app_secret: &str) -> bool {
        // 正向:从定时器发送数据,并到ws接收数据
        let (tx, _rx) = mpsc::unbounded_channel::<ByteString>();
        // 反向:从ws接收,并传输到定时器
        let (reverse_tx, _reverse_rx) = mpsc::unbounded_channel::<WsResponse>();
        let mut rx = UnboundedReceiverStream::new(_rx);
        let mut reverse_rx = UnboundedReceiverStream::new(_reverse_rx);

        let token = CancellationToken::new();
        let cloned_token = token.clone();
        let myid = config::get_server_id();

        // 启动定时发送任务
        let send_handle = tokio::spawn(async move {
            // 3秒后开始算
            let start_at = tokio::time::Instant::now() + START_AFTER;
            // 间隔 10 秒执行一次
            let mut intv = time::interval_at(start_at, INTERVAL);

            log::debug!(
                "[{}] - [{}] - Sender initialized and will start after {:?}, running every {:?}",
                &myid,
                &server_id,
                START_AFTER,
                INTERVAL
            );
            // 投票箱

            // 加入后进入投票环节
            let mut event;
            let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();

            loop {
                select! {

                    _ = cloned_token.cancelled() => {
                        break;
                    }
                    Some(v) = reverse_rx.next() => {
                        // 数据分析
                        match v.event {
                            ws::WsEvent::Offline => {
                                // Follower下线
                                log::info!("[{}] - [{}] - Follower off-line, Nothing to do", &myid, &server_id);
                            }
                            ws::WsEvent::LeaderOffline => {
                                // leader下线
                                log::info!("[{}] - [{}] - Leader off-line", &myid, &server_id);
                                let mut sga = lock.lock();
                                // 重新开始投票
                                sga.reset_with_vote();
                            }
                            ws::WsEvent::Vote => {
                                // 投票
                                match v.result {
                                    // 投票成功
                                    WsResult::Ok => {
                                        if let Some(vcd) = v.vcd {
                                            log::debug!("[{}] - [{}] - Voting completed", &myid, &server_id);
                                            let mut sga = lock.lock();
                                            // 我选择弃权
                                            sga.released = true;
                                            // 设置我的票数去向
                                            sga.poll_to = Some(VTo::new(myid, sga.vcd.poll));
                                            // 返回最终的票数，正常 vcd.poll=0
                                            sga.vcd.poll = vcd.poll;
                                            // 通知其他等待的线程
                                            cvar.notify_one();
                                        }
                                    }
                                    WsResult::Failed=> {
                                        // 投票失败
                                        log::debug!("[{}] - [{}] - Voting failed", &myid, &server_id);
                                    }
                                    // ERROR
                                    _ => {}

                                }

                            }

                            ws::WsEvent::Changed => {
                                log::debug!("[{}] - [{}] - Voting will change", &myid, &server_id);
                                let mut sga = lock.lock();
                                sga.changed_poll(server_id);
                            }
                            ws::WsEvent::Leader => {
                                if let Some(vcd) = v.vcd {
                                    // 收到广播数据
                                    let mut sga = lock.lock();
                                    if !sga.is_not_looking() {
                                        // 同步数据
                                        sga.sync_leader(vcd.leader, vcd.term, shared::Status::Following);
                                        log::debug!("[{myid}] - [{server_id}] - RECV Leader Broadcast, ga = {}", sga.fmt());
                                    }
                                }
                            }

                            // 心跳完成
                            ws::WsEvent::Heartbeat => {
                                log::debug!("[{}] - [{}] - Heartbeat Ok", &myid, &server_id);
                            }

                            _ => { }
                        }
                    }
                    _ = intv.tick() => {
                        // 每隔一段时间运行一次

                        // 没有票数的时候，就等待
                        let mut sga = lock.lock();
                        // 没有leader
                        if !sga.is_not_looking() {

                            // if sga.vcd.term > 0 {
                            //     // 再次再试投票
                            //     event = WsEvent::Vote;
                            // }

                            // log::debug!("[{myid}] - [{server_id}] - Voter.SGA:tick: {:?}", sga);
                            // 如果已经放弃候选人的角色，则后续无需参与投票
                            if sga.released {
                                log::debug!("[{myid}] - [{server_id}] - Changing event, current status is abandon, sleep for {:?} to continue", INTERVAL);
                                continue;
                            }

                            // 投票来源谁？无需重复投票
                            if sga.is_poll_from(&server_id) {
                                log::debug!("[{myid}] - [{server_id}] - Changing event, Voted from {server_id}, sleep for {:?} to continue", INTERVAL);
                                continue;
                            }
                            // 我投票给谁？无需重复投票
                            if sga.is_poll_to(&server_id) {
                                log::debug!("[{myid}] - [{server_id}] - Changing event, Voted to {server_id}, sleep for {:?} to continue", INTERVAL);
                                continue;
                            }

                            // 投票转移, 已经投票过,且投票失败
                            if sga.get_vcd().poll_changed.contains(&server_id) {
                                log::debug!("[{myid}] - [{server_id}] - Changed Vote, sleep for {:?} to continue", INTERVAL);
                                continue;
                            }


                            if sga.vcd.poll == 0 {
                                log::debug!("[{myid}] - [{server_id}] - sga.poll == 0, cvar.wait...");
                                // 10秒后超时，不等待
                                if cvar.wait_until(&mut sga, Instant::now() + LOCK_TIMEOUT).timed_out() {
                                    log::debug!("[{myid}] - [{server_id}] - sga.poll == 0, cvar.wait...timeout...");
                                    continue;
                                }
                            }
                            event = ws::WsEvent::Vote
                        } else {
                            event =  ws::WsEvent::Sync
                        }

                        if myid == server_id {
                            event = ws::WsEvent::Heartbeat
                        }

                        // 连接123，数据重装，将票数来源
                        let src = WsRequest{
                            event: event.clone(),
                            vcd: sga.get_vcd()
                        }.to_bytestr();

                        if let Err(e) = tx.send(src) {
                            log::error!("[{myid}] - [{server_id}] - TX send failed, cause: {e}");
                            break;
                        } else {
                            log::debug!("[{myid}] - [{server_id}] - TX sent, sleep for {:?} to continue", INTERVAL);
                        }
                    }
                }
            }
        });

        let url = format!(
            "ws://{}:{}/im/chat/{}/{}",
            self.addr, self.port, server_id, myid
        );
        let (_resp, mut ws) = match awc::Client::new()
            .ws(&url)
            .header(servlet::imguard::HEADER_APP_KEY, app_key)
            .header(servlet::imguard::HEADER_APP_SECRET, app_secret)
            .connect()
            .await
        {
            Ok((a, b)) => (a, b),
            Err(e) => {
                log::error!(
                    "[{myid}] - [{server_id}] - Failed to connect to {}:{}, cause: {e}",
                    self.addr,
                    self.port
                );

                // 如果连接失败的是leader
                let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
                let mut sga = lock.lock();
                if sga.is_leader(server_id) {
                    // 重置，开始投票
                    sga.reset_with_vote();
                    log::debug!("[{myid}] - [{server_id}] - Leader offline, Start voting leader");
                }
                cvar.notify_one();
                token.cancel();
                log::debug!("[{}] - [{}] - Sender cancelled", &myid, &server_id,);
                return true;
            }
        };
        log::info!(
            "[{myid}] - [{server_id}] - Connection established to {}:{}",
            self.addr,
            self.port
        );

        loop {
            tokio::select! {
                // 接收信息
                Some(payload) = ws.next() => {
                    let payload = match payload {
                        Ok(f) => f,
                        Err(_) => return false
                    };
                    match payload {
                        Frame::Text(msg) => {

                            // 接收数据解析
                            let raw = String::from_utf8_lossy(msg.as_ref()).to_string();
                            log::debug!("[{myid}] - [{server_id}] - RECV data <= {raw}");

                            let v = match WsResponse::from_str(&raw) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!("[{myid}] - [{server_id}] - WsParseError: `{raw}` is not valid JSON, cause: {e}");
                                    return false
                                }
                            };
                            // 状态码判断
                            match v.result {
                                WsResult::Error => {
                                    log::error!("[{myid}] - [{server_id}] - The server {server_id} returned an error: {}", v.message);
                                    return false
                                }
                                _ => {
                                }
                            }

                            // 将数据传回定时器
                            if let Err(e) = reverse_tx.send(v) {
                                log::error!("[{myid}] - [{server_id}] - RTX send failed, cause: {e}");
                            }
                        }
                        Frame::Ping(msg) => {
                            // log::debug!("[{}] - - Ping: {:?}", id, msg);
                            if let Err(e) = ws.send(Message::Pong(msg)).await {
                                log::error!("[{myid}] - [{server_id}] - Ping failed, cause: {e}");
                            }
                            // ws.send(Message::Pong(msg)).await.unwrap();
                        }
                        Frame::Pong(msg) => {
                            log::debug!("[{myid}] - [{server_id}] - Pong: {:?}", msg);
                        }
                        Frame::Continuation(msg) => {
                            log::debug!("[{myid}] - [{server_id}] - Continuation: {:?}", msg);
                        }
                        Frame::Close(msg) => {
                            log::debug!("[{myid}] - [{server_id}] - Connection closed, cause: {}", msg.unwrap().description.unwrap());
                            break;
                        }
                        Frame::Binary(msg) => {
                            log::debug!("[{myid}] - [{server_id}] - Binary: {:?}", msg);
                        }
                    }
                },

                // 定时发送信息
                Some(buf) = rx.next() => {
                    if let Err(e) = ws.send(Message::Text(buf.clone())).await {
                        log::info!("[{myid}] - [{server_id}] - Unable to send to {server_id}, cause: {e}");
                        break;
                    } else {
                        log::debug!("[{myid}] - [{server_id}] - Sent data => {buf}");
                    }
                },

                else => {
                    // 未匹配
                }

            }
        }

        // drop
        token.cancel();

        // 将send_handle添加到任务中
        let _ = tokio::join!(send_handle);

        false
    }
}
