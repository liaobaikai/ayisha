use std::{sync::Arc, time::Duration};

use awc::ws::{Frame, Message};
use bytestring::ByteString;
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::{select, sync::mpsc::{self}, time::{self, Instant}};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::{config, pom::{PoManager, State}, servlet, shared, ws::{self, WsEvent, WsRequest, WsResponse, WsStatusCode}};

pub struct VoteHandler {
    addr: String,
    port: u16,
    server_id: usize,
}

impl VoteHandler {

    pub fn new(addr: String, port: u16, server_id: usize) -> Self {

        VoteHandler {
            addr,
            port,
            server_id,
        }

    }

    // 启动
    pub async fn start(&self, app_key: &str, app_secret: &str) -> bool {
        
        // 正向:从定时器发送数据,并到ws接收数据
        let (tx, _rx) = mpsc::unbounded_channel::<ByteString>();
        // 反向:从ws接收,并传输到定时器
        let (reverse_tx, _reverse_rx) = mpsc::unbounded_channel::<WsResponse>();
        let mut rx = UnboundedReceiverStream::new(_rx);
        let mut reverse_rx = UnboundedReceiverStream::new(_reverse_rx);
        // let tx1 = tx.clone();

        let token = CancellationToken::new();
        let cloned_token = token.clone();
        // let inner_id = String::from(id);
        let server_id = self.server_id;
        // let server_id = format!("{}", self.server_id);
        // let target_id = Arc::new(format!("{}", self.server_id));
        // let cloned_target_id = Arc::clone(&target_id);
        let cloned_server_id = &server_id.clone();
        // let inner_id2 = String::from(id);
        
        // let mut c_attr = self.pom.clone();
        // let mut pom = self.pom.clone();
        // pom.v_state.id = self.server_id;
        // let voter_id = pom.v_state.id;
        // let cloned_voter_id = voter_id.clone();
        
        
        // 启动定时发送任务
        let send_handle = tokio::spawn(async move {
            // 3秒后开始算
            let start_at = Instant::now() + Duration::from_secs(3);
            // 间隔 10 秒执行一次
            let interval = Duration::from_secs(10);
            let mut intv = time::interval_at(start_at, interval);
            log::debug!("[{}] - [{}] - Voter Timer start at: {:?}, interval {:?}", &voter_id, &server_id, start_at, interval);
            // 投票箱
            
            // 加入后进入投票环节
            let mut event = WsEvent::VOTE;
            // 已经过了投票环节
            // if pom.v_state.poll == 0 {
            //     event = WsEvent::JOIN;
            // }
            // if server_id == voter_id {
            //     // 只需要同步数据即可。
            //     // 从磁盘中将数据写入内存中
            //     // Voter -> Candidate
            //     // 主动同步数据
            //     event = WsEvent::COPY;
            // }

            let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();

            loop {
                select! {
                    _ = cloned_token.cancelled() => {
                        break;
                    }
                    Some(v) = reverse_rx.next() => {
                        // v => WsResponse
                        // log::debug!("[{}] - [{}] - RRX recv: {:?}", &voter_id, &server_id, v);
                        // 数据分析
                        match v.event {
                            ws::WsEvent::JOIN => {
                                // 进入数据同步
                                if let Some(_state) = v.state {
                                    // c_attr.poll = data.poll;
                                    

                                    // match state.status {
                                    //     WsDataStatus::NONTODO => {
                                    //         continue
                                    //     }

                                    //     WsDataStatus::RECV | WsDataStatus::SEND => {
                                    //         // 进入数据同步环节
                                    //         // c_attr.event = ws::WsEvent::COPY;
                                    //         // let src = c_attr.to_bytestr();
                                    //         // if let Err(e) = tx.send(src) {
                                    //         //     log::error!("[{}] - - TX send failed, cause: {}", &inner_id, e);
                                    //         //     break;
                                    //         // }
                                    //     }
                                    // }
                                }
                            }
                            ws::WsEvent::COPY => {
                                // 其他节点同步数据到本节点
                                if let Some(_state) = v.state {
                                    if server_id == voter_id {
                                        // 同步完成
                                        log::debug!("[{}] - [{}] - Copy with candidate done, nothing to do", &voter_id, &server_id);
                                        // 刷新到本地
                                        pom.v_flush();

                                        // if let Ok(mut g) = POLL_MUTEX.try_lock() {
                                        //     *g += 1;
                                        // }
                  

                                    }
                                    // match data.status {
                                    //     WsDataStatus::NONTODO => {
                                    //         continue
                                    //     }

                                    //     WsDataStatus::RECV => {
                                    //         // 数据需要从节点节点同步到本节点
                                    //         // local_request.event = WsEvent::COPY;
                                    //         log::debug!("[{}] ### RECV data:: {:?}", &server_id, data);

                                    //     }

                                    //     WsDataStatus::SEND => {
                                    //         // 数据同步到其他节点
                                    //     }
                                    // }
                                }
                            }
                            ws::WsEvent::VOTE => {
                                // 投票
                                match v.status_code {
                                    // 投票成功
                                    WsStatusCode::SUCCESS => {
                                        if let Some(state) = v.state {
                                            log::debug!("[{}] - [{}] - Voter data updated, change event to <JOIN>", &voter_id, &server_id);
                                            // 投票已提交
                                            pom.v_state = state;
                                            // 修改事件类型
                                            event = WsEvent::JOIN;
                                            // 刷盘
                                            pom.v_flush();
                                        }
                                    }
                                    WsStatusCode::FAILED=> {
                                        // 投票失败
                                        // 获取所有票数，投给其他候选人
                                        // 
                                        if let Some(state) = v.state {
                                            log::debug!("[{}] - [{}] - Changed vote, change event to <CHANGED>", &voter_id, &server_id);
                                            // 投票已提交
                                            pom.v_state.poll = state.poll;
                                            // 修改事件类型
                                            event = WsEvent::CHANGED;
                                            // 刷盘
                                            // if voter_id == server_id {
                                            //     // 服务端票数返回，马上刷盘
                                            pom.v_flush();
                                            // }
                                        }
                                    }

                                    // ERROR
                                    _ => {}
                                    
                                }

                            }


                            ws::WsEvent::CHANGED => {
                                // 投票转移
                                // match v.status_code {
                                //     WsStatusCode::SUCCESS => {
                                //         // 投票已提交
                                //         pom.v_state.poll = 0;
                                //         // 修改事件类型
                                //         event = WsEvent::JOIN;
                                //         // 刷盘
                                //         pom.v_flush();

                                //     }
                                //     WsStatusCode::FAILED=> {
                                //         // 投票失败
                                //         // 获取所有票数，投给其他候选人

                                //     }

                                //     // ERROR
                                //     _ => {}
                                    
                                // }

                            }

                            ws::WsEvent::BROADCAST => {
                                // 接收广播数据（投票传递）
                                if let Some(_state) = v.state {
                                    // // 判断投票情况
                                    // // 1、如果对方的数据比我要新，则需要改票
                                    // if data.tranx > c_attr.tranx {
                                    //     // 对方的数据比我新
                                    // }

                                }
                            }

                            ws::WsEvent::UNKNOWN => {
                            }
                        }
                    }
                    _ = intv.tick() => {
                        // 每隔一段时间运行一次
                        let state = pom.get_vdata();
                        let event = event.clone();

                        let src = WsRequest{
                            event,
                            state
                        }.to_bytestr();

                        if let Err(e) = tx.send(src) {
                            log::error!("[{voter_id}] - [{server_id}] - TX send failed, cause: {e}");
                            break;
                        } else {
                            log::debug!("[{voter_id}] - [{server_id}] - TX send, sleep for 10 seconds to continue");
                        }
                    }
                }
            }
        });


        let url = format!("ws://{}:{}/im/chat/{}/{}/{}", self.addr, self.port, config::get_client_namespace(), self.server_id, cloned_voter_id);
        let (_resp, mut ws) = match awc::Client::new().ws(&url)
                .header(servlet::imguard::HEADER_APP_KEY, app_key)
                .header(servlet::imguard::HEADER_APP_SECRET, app_secret).connect().await {
            Ok((a, b)) => (a, b),
            Err(e) => {
                log::error!("[{cloned_voter_id}] - [{cloned_server_id}] - Failed to connect to {}:{}, cause: {e}", self.addr, self.port);
                token.cancel();
                return true
            }
        };
        log::info!("[{cloned_voter_id}] - [{cloned_server_id}] - Connection established to {}:{}", self.addr, self.port);
        
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
                            log::debug!("[{}] - [{}] - Received data from the server {}, `{}`", cloned_voter_id, cloned_server_id, cloned_server_id, raw);
                            let v = match WsResponse::from_str(&raw) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!("[{cloned_voter_id}] - [{cloned_server_id}] - WsParseError: `{raw}` is not valid JSON, cause: {e}");
                                    return false
                                }
                            };
                            // 状态码判断
                            match v.status_code {
                                WsStatusCode::SUCCESS | WsStatusCode::FAILED=> {
                                }
                                WsStatusCode::ERROR => {
                                    log::error!("[{cloned_voter_id}] - [{cloned_server_id}] - The server {cloned_server_id} returned an error: {}", v.mail_box);
                                    return false
                                }
                            }
                            
                            // 将数据传回定时器
                            if let Err(e) = reverse_tx.send(v) {
                                log::error!("[{cloned_voter_id}] - [{cloned_server_id}] - RTX send failed, cause: {e}");
                            }
                        }
                        Frame::Ping(msg) => {
                            // log::debug!("[{}] - - Ping: {:?}", id, msg);
                            if let Err(e) = ws.send(Message::Pong(msg)).await {
                                log::error!("[{cloned_voter_id}] - [{cloned_server_id}] - Ping failed, cause: {e}");
                            }
                            // ws.send(Message::Pong(msg)).await.unwrap();
                        }
                        Frame::Pong(msg) => {
                            log::debug!("[{cloned_voter_id}] - [{cloned_server_id}] -  Pong: {:?}", msg);
                        }
                        Frame::Continuation(msg) => {
                            log::debug!("[{cloned_voter_id}] - [{cloned_server_id}] - Continuation: {:?}", msg);
                        }
                        Frame::Close(msg) => {
                            log::debug!("[{cloned_voter_id}] - [{cloned_server_id}] - Connection closed, cause: {}", msg.unwrap().description.unwrap());
                            break;
                        }
                        Frame::Binary(msg) => {
                            log::debug!("[{cloned_voter_id}] - [{cloned_server_id}] - Binary: {:?}", msg);
                        }
                    }
                },

                // 定时发送信息
                Some(buf) = rx.next() => {
                    log::debug!("[{cloned_voter_id}] - [{cloned_server_id}] - Send to the server {cloned_server_id}, `{buf}`");
                    if let Err(e) = ws.send(Message::Text(buf)).await {
                        log::info!("[{cloned_voter_id}] - [{cloned_server_id}] - Unable to send to the server {cloned_server_id}, cause: {e}");
                        break;
                    }
                },

            }

        }

        // drop
        token.cancel();

        // 将send_handle添加到任务中
        let _ = tokio::join!(send_handle);

        false

    }
}

