use crate::{
    config,
    shared::{self, VCData, VFrom},
    ws::{WsEvent, WsResponse},
};
use actix::prelude::*;
use rand::{rngs::ThreadRng, Rng};
use std::{collections::HashMap, net::IpAddr};
// https://github.com/actix/examples/blob/master/websockets/chat/src/server.rs
// https://cloud.tencent.com/developer/article/1756850

/// Chat server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

/// Message for chat server communications

/// New chat session is created
#[derive(Message, Debug)]
#[rtype(usize)]
pub struct Connect {
    // pub id: usize,
    pub myid: usize,
    pub voter_id: usize,
    pub addr: Recipient<Message>,
    pub ip: IpAddr,
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub session_id: usize,
    pub myid: usize,
    pub voter_id: usize,
    // pub namespace: String,
    /// Is closed connection?
    pub closed: bool,
    pub ip: IpAddr,
}

/// Send message to specific cluster
#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    /// Id of the client session
    pub id: usize,
    /// Peer message
    pub msg: String,
    /// cluster name
    pub cluster: String,
}

/// Join cluster, if room does not exists create new one.
#[derive(Message)]
#[rtype(Join)]
pub struct Join {
    pub session_id: usize,
    pub vcd: VCData,
}

// 数据同步
#[derive(Message)]
#[rtype(Copy)]
pub struct Copy {
    pub session_id: usize,
    pub vcd: VCData,
}

// 数据同步
#[derive(Message)]
#[rtype(result = "()")]
pub struct Reset {
    pub session_id: usize,
}

// 节点内部通讯
#[derive(Message)]
#[rtype(Heartbeat)]
pub struct Heartbeat {
    pub session_id: usize,
    pub vcd: VCData,
    pub hb_failed_count: usize,
    pub result: HbResult,
}

#[derive(Debug)]
pub enum HbResult {
    Undefined,
    Election,
    Ok,
    Follower,
}

// 投票
#[derive(Debug, Message)]
#[rtype(Vote)]
pub struct Vote {
    pub session_id: usize,
    // 实际数据
    pub vcd: VCData,
    // 锁等待超时
    pub timeout: bool,
    pub result: VoteResult,
}

#[derive(Debug)]
pub enum VoteResult {
    // 未定义的
    Undefined,
    // 投票成功
    Ok,
    // 投票转换
    Change,
    // 投票拒绝，当前的角色不接受投票
    Abandon,
    // 刚加入的节点,已经有leader了
    Leader,
}

/// `ChatServer` manages chat rooms and responsible for coordinating chat session.
///
/// Implementation is very naïve.
#[derive(Debug)]
pub struct ChatServer {
    // SESSION_ID, ADDR
    sessions: HashMap<usize, Recipient<Message>>,
    // SESSION_ID, UID
    // namespaces: HashMap<String, HashSet<usize>>,
    rng: ThreadRng,
    // VOTER_ID, POLL
    vote_from: HashMap<usize, usize>,
    // SESSION_ID, VOTER_ID
    session_keys: HashMap<usize, usize>,
    // 连接失败数统计，超过3次，则降级
    hb_failed_count: usize,
}

impl ChatServer {
    pub fn new() -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
            // namespaces: HashMap::new(),
            rng: rand::thread_rng(),
            vote_from: HashMap::new(),
            session_keys: HashMap::new(),
            hb_failed_count: 0,
        }
    }
}

impl ChatServer {

    // 选举领导者
    // 超过超过半数才开始选举
    fn electing_leader(&mut self, mut sga: parking_lot::lock_api::MutexGuard<parking_lot::RawMutex, shared::GlobalArea>, mut msg: Heartbeat) -> Heartbeat {
        
        if sga.released {
            return msg;
        }
        // 有状态则不用选举
        if sga.is_not_looking() {
            return msg;
        }

        // 重新选举 term > 0
        // 首次选举 term = 0
        if sga.vcd.term > 0 {
            // 重新选举
            self.vote_from = HashMap::new();
            log::debug!(
                "[{}] - [{}] - Reset vote_from...",
                sga.vcd.myid,
                sga.vcd.myid
            );
        }

        ///////////////////////////////////////////////////////////////////
        // 选举开始
        ///////////////////////////////////////////////////////////////////
        // 总节点数: 3
        let node_count = config::get_nodes().len();
        // 最小存活节点
        let expect_min_active_count = if node_count % 2 == 1 {
            (node_count + 1) / 2
        } else {
            node_count / 2
        };
        
        // 支持者数量: + 自身1票
        let current_active_count = (self.vote_from.keys().len() | sga.vcd.poll_from.len()) + 1;
        // 至少超过一半存活，否则等待
        if current_active_count < expect_min_active_count {
            // 需要等待其他节点
            log::info!("[{}] - [{}] - Lack of voting conditions, current active count is {}, expect min active count is {}, waiting", sga.vcd.myid, sga.vcd.myid, current_active_count, expect_min_active_count);
            return msg;
        }

        

        // 票数也正确
        if sga.vcd.poll >= expect_min_active_count {
            // 本节点作为leader
            // 更新自身信息
            let leader = sga.vcd.myid;
            let term = sga.vcd.term + 1;
            sga.sync_leader(leader, term, shared::Status::Leading);

            // 选举成功
            msg.result = HbResult::Election;

            // 同步给客户端
            msg.vcd.sync_leader(leader, term);

            // 广播给其他节点，通知其他节点leader已选出
            for (sid, addr) in self.sessions.iter() {
                if let Some(vid) = self.session_keys.get(&sid) {
                    if *vid == sga.vcd.myid {
                        continue;
                    }

                    log::debug!(
                        "[{}] - [{}] - Election completed, leader is {}, broadcasting to session {}",
                        sga.vcd.myid,
                        vid,
                        leader,
                        sid
                    );
                    let message = WsResponse::ok(
                        WsEvent::Leader,
                        "Copy to Voter".to_owned(),
                        Some(msg.vcd.clone()),
                    )
                    .to_string();
                    addr.do_send(Message(message));
                }
            }
        }

        msg
    }
}

/// Make actor from `ChatServer`
impl Actor for ChatServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<Connect> for ChatServer {
    type Result = usize;

    // 生成session_id
    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        let session_id: u64 = self.rng.gen::<u64>();
        let session_id = session_id as usize;
        self.sessions.insert(session_id, msg.addr);
        self.session_keys.insert(session_id, msg.voter_id);
        log::debug!(
            "[{}] - [{}] - Session created, session ID is {}",
            msg.myid,
            msg.voter_id,
            session_id
        );
        // send id back
        session_id
    }
}

// Handler for join message.
impl Handler<Join> for ChatServer {
    type Result = MessageResult<Join>;
    // 普通节点恢复
    // 原leader节点恢复后降级为普通节点

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) -> Self::Result {
        MessageResult(msg)
    }
}

// Handler for Reset message.
impl Handler<Reset> for ChatServer {
    type Result = ();
    fn handle(&mut self, _msg: Reset, _: &mut Context<Self>) {
        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let mut sga: parking_lot::lock_api::MutexGuard<parking_lot::RawMutex, shared::GlobalArea> = lock.lock();
        sga.reset_with_vote();
        self.vote_from = HashMap::new();
        cvar.notify_one();
    }
}

// 数据同步
impl Handler<Copy> for ChatServer {
    type Result = MessageResult<Copy>;
    fn handle(&mut self, msg: Copy, _: &mut Context<Self>) -> Self::Result {
        MessageResult(msg)
    }
}

// 数据同步
impl Handler<Heartbeat> for ChatServer {
    type Result = MessageResult<Heartbeat>;
    fn handle(&mut self, mut msg: Heartbeat, _: &mut Context<Self>) -> Self::Result {

        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let sga = lock.lock();
        match sga.status {
            shared::Status::Looking => {
                let msg = self.electing_leader(sga, msg);
                cvar.notify_one();
                return MessageResult(msg);
            },
            shared::Status::Leading => {
                if msg.hb_failed_count > 0 && msg.hb_failed_count != self.hb_failed_count {
                    // 数据改变了，且超过3次，则降级为普通节点
                    // 总节点数: 3
                    let node_count = config::get_nodes().len();
                    // 最小存活节点
                    let expect_min_active_count = if node_count % 2 == 1 {
                        (node_count + 1) / 2
                    } else {
                        node_count / 2
                    };

                    // 至少超过一半存活，否则认为是孤岛
                    if msg.hb_failed_count < expect_min_active_count {
                        cvar.notify_one();
                        return MessageResult(msg);
                    }
                    // 存活不过半了，将自己降级
                    msg.result = HbResult::Follower;
                    self.vote_from = HashMap::new();
                    
                    cvar.notify_one();
                    return MessageResult(msg);
                }
                self.hb_failed_count = msg.hb_failed_count;

            }
            _ => {}
        }

        msg.result = HbResult::Ok;

        cvar.notify_one();
        return MessageResult(msg);
    }
}

// leader竞选
// 1.term大的直接胜出
// 2.term相同，事务id大的胜出
// 3.事务id相同，服务器id大的胜出
impl Handler<Vote> for ChatServer {
    type Result = MessageResult<Vote>;

    fn handle(&mut self, mut msg: Vote, _: &mut Context<Self>) -> Self::Result {
        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let mut sga = lock.lock();

        // 有状态则不用选举
        if sga.is_not_looking() {
            msg.result = VoteResult::Leader;
            log::debug!("[{}] - [{}] - Join At Follower", sga.vcd.myid, msg.vcd.myid);
            // 同步给投票人
            msg.vcd.sync_leader(sga.vcd.leader, sga.vcd.term);
            return MessageResult(msg);
        }

        if sga.vcd.poll == 0 {
            // 拒绝投票
            msg.result = VoteResult::Abandon;
            log::debug!(
                "[{}] - [{}] - Abandoned Candidate",
                sga.vcd.myid,
                msg.vcd.myid
            );
            return MessageResult(msg);
        }

        let myid = sga.vcd.myid;
        let vid = msg.vcd.myid;
        // 候选人：投票周期
        let term = sga.vcd.term;
        // 投票人：投票周期
        let voter_term = msg.vcd.term;

        // let poll = sga.vcd.poll;
        let voter_poll = msg.vcd.poll;

        // 是否投票转移
        let mut change = false;
        if term > voter_term {
            // pass
        } else if term < voter_term {
            // 候选人失去候选机会，投票失败，票数转移
            change = true;
            log::debug!(
                "[{}] - [{}] - Voting change actively, candidate term is {}, voter term is {}",
                sga.vcd.myid,
                msg.vcd.myid,
                term,
                voter_term
            );
        } else {
            // 投票周期一致
            // 判断tranx，tranx越大，则越新
            // 事务数
            let tranx = sga.vcd.tranx;
            let voter_tranx = msg.vcd.tranx;

            // 比较事务数，事务数越大，则代表该节点的数据越新
            if tranx > voter_tranx {
                // 本机作为leader
            } else if tranx < voter_tranx {
                // 候选人失去候选机会，投票失败，票数转移
                change = true;
                log::debug!("[{}] - [{}] - Voting change actively, candidate tranx is {}, voter tranx is {}", sga.vcd.myid, msg.vcd.myid, tranx, voter_tranx);
            } else {
                // 比对 ID，ID 越大，就投票给大的
                if myid < vid {
                    // 候选人失去候选机会，投票失败，票数转移
                    change = true;
                    log::debug!(
                        "[{}] - [{}] - Voting change actively, candidate id is {}, voter id is {}",
                        sga.vcd.myid,
                        msg.vcd.myid,
                        myid,
                        vid
                    );
                }
            }
        }

        // 票数获得:
        // 方式1:投票失败后,通过投票将自身的票数带回(存在缺陷)
        // 方式2:投票失败后,返回投票失败消息,通过voter正常投票
        if change {
            msg.result = VoteResult::Change;
        } else {
            // 投票确认
            sga.vcd.poll += voter_poll;
            // 清空投票者的票数
            msg.vcd.poll = 0;
            // 投票节点数,
            // 如果存在多次投票,最后一次会覆盖前面的投票情况
            self.vote_from.insert(msg.vcd.myid, voter_poll);

            log::debug!(
                "[{}] - [{}] - Voting completed, acquired votes {}, total votes {}",
                sga.vcd.myid,
                msg.vcd.myid,
                voter_poll,
                sga.vcd.poll
            );

            // 正常投票，投票来源
            for vv in msg.vcd.poll_from.clone() {
                sga.poll_from(vv);
            }

            let vfrom = VFrom::new(msg.vcd.myid, voter_poll);
            sga.poll_from(vfrom.clone());
            msg.vcd.poll_from.push(vfrom);

            msg.result = VoteResult::Ok;
        }

        cvar.notify_one();

        return MessageResult(msg);
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        if msg.closed || msg.voter_id == msg.myid {
            return;
        }

        let myid = msg.myid;
        let leader;
        {
            let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
            let sga = lock.lock();
            leader = sga.vcd.leader;
            cvar.notify_one();
        }

        if myid == leader {
            // leader异常，则无需打印，且无法广播数据到其他节点
            return;
        }

        // 从会话列表中删除
        let _removed_vid = match self.sessions.remove(&msg.session_id).and_then(|_| {
            self.session_keys.remove(&msg.session_id)
        }) {
            Some(v) => {
                log::info!(
                    "[{}] - [{}] - Session {} removed",
                    msg.myid,
                    msg.voter_id,
                    msg.session_id,
                );
                v
            },
            None => return,
        };

        // 通知其他会话，排除本地心跳会话
        // session_keys: session_id: voter_id
        for (sid, addr) in self.sessions.iter() {
            let vid = match self.session_keys.get(&sid) {
                None => continue,
                Some(id) => id,
            };

            // 本地连接
            if *vid == myid {
                continue;
            }

            // 针对本机使用CTRL+C停止，且本机非leader时，会提示“Leader off-lined, broadcast to...”
            // 解决方式：取消signal：CTRL+C
            let message = if leader == msg.voter_id {
                log::debug!(
                    "[{myid}] - [{}] - Leader off-lined, broadcast to {vid} of session {sid}", msg.voter_id
                );
                WsResponse::ok(WsEvent::LeaderOffline, "Leader Offline".to_owned(), None)
            } else {
                log::debug!(
                    "[{myid}] - [{}] - Follower off-lined, broadcast to {vid} of session {sid}", msg.voter_id
                );
                WsResponse::ok(WsEvent::Offline, "Offline".to_owned(), None)
            };
            
            addr.do_send(Message(message.to_string()));
        }
    }
}
