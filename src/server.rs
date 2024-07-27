use actix::prelude::*;
use parking_lot::{Condvar, Mutex};
use rand::{rngs::ThreadRng, Rng};
use std::{
    clone,
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::{
    config,
    shared::{self, VCData, VFrom, VTo},
    ws::{self, WsEvent, WsRequest, WsResponse},
};
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
    pub namespace: String,
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
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
#[rtype(result = "()")]
pub struct Private {
    pub session_id: usize,
    pub namespace: String,
    pub vcd: VCData,
}

// 投票
#[derive(Debug, Message)]
#[rtype(Vote)]
pub struct Vote {
    pub session_id: usize,
    // 实际数据
    pub vcd: VCData,
    // 是否投票成功
    // pub ok: bool,
    // 锁等待超时
    pub timeout: bool,
    // 命名空间
    pub namespace: String,
    //
    pub result: VoteResult,
}

#[derive(Debug)]
pub enum VoteResult {
    // 未定义的
    UNDEFINED,
    // 投票成功
    SUCCESS,
    // 投票转换
    CHANGE,
    // 投票拒绝，当前的角色不接受投票
    ABANDON,
    // 刚加入的节点,已经有leader了
    LEADER,
}

/// `ChatServer` manages chat rooms and responsible for coordinating chat session.
///
/// Implementation is very naïve.
#[derive(Debug)]
pub struct ChatServer {
    // SESSION_ID, ADDR
    sessions: HashMap<usize, Recipient<Message>>,
    // SESSION_ID, UID
    // session_map: HashMap<usize, usize>,
    namespaces: HashMap<String, HashSet<usize>>,
    // visitor_count: Arc<AtomicUsize>,
    rng: ThreadRng,
    // pom: PoManager,

    // pair: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, bool>, Condvar)>,

    // 投票来源: voter_id, poll (投票人 ID，票数)
    // poll_from: HashMap<usize, usize>,
    // 候选人状态信息
    // c_state: State,
    // 本地连接
    // local_session_list: Vec<usize>,
    // (投票人 ID，票数)
    vote_from: HashMap<usize, usize>,
}

impl ChatServer {
    // static GLOBAL_STATE_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
    pub fn new() -> ChatServer {
        // default namespaces
        // let namespaces = HashMap::new();
        // namespaces.insert(ns.clone(), HashSet::new());
        // let pair = Arc::new((Mutex::new(false), Condvar::new()));
        // let pair2 = GLOBAL_STATE_MUTEX_PAIR.clone();
        // let &(ref lock, ref cvar) = &*pair2;

        ChatServer {
            sessions: HashMap::new(),
            // session_map: HashMap::new(),
            namespaces: HashMap::new(),
            // visitor_count,
            rng: rand::thread_rng(),
            // pair,
            // pom,
            // poll_from: HashMap::new(),
            // c_state: State::new(),
            // local_session_list: Vec::new()
            vote_from: HashMap::new(),
        }
    }
}

impl ChatServer {
    /// Send message to all users in the cluster
    fn send_message(&self, namespace: &str, message: &str, skip_sid: usize) {
        if let Some(sessions) = self.namespaces.get(namespace) {
            for sid in sessions {
                if *sid != skip_sid {
                    if let Some(addr) = self.sessions.get(sid) {
                        addr.do_send(Message(message.to_owned()));
                    }
                }
            }
        }
    }

    // 选举领导者
    // 超过超过半数才开始选举
    fn electing_leader(&mut self, mut msg: Private) {
        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let mut sga = lock.lock();
        if sga.released {
            cvar.notify_one();
            return;
        }
        // 有状态则不用选举
        match sga.status {
            shared::Status::LEADING | shared::Status::FOLLOWING => return,
            _ => {}
        }
        // {
        //     for v in sga.vcd.poll_from.iter() {
        //         if !self.vote_from.contains_key(&v.from_id) {
        //             self.vote_from.insert(v.from_id, v.poll);
        //         }
        //     }
        // }
        ///////////////////////////////////////////////////////////////////
        // 选举开始
        ///////////////////////////////////////////////////////////////////
        // 总节点数: 3
        let node_count = config::get_nodes().len();
        // 最小存活节点
        let min_active_count = if node_count % 2 == 1 {
            (node_count + 1) / 2
        } else {
            node_count / 2
        };
        // 支持者数量: + 自身1票
        let count = (self.vote_from.keys().len() | sga.vcd.poll_from.len()) + 1;
        // 至少超过一半存活
        if count < min_active_count {
            // 需要等待其他节点
            log::info!("[{}] - [{}] - Non election conditions, insufficient voting node, waitting for other node join...", sga.vcd.myid, sga.vcd.myid);
            return;
        }

        // 票数也正确
        if sga.vcd.poll >= min_active_count {
            // 本节点作为leader
            // 更新自身信息
            sga.status = shared::Status::LEADING;
            sga.vcd.leader = sga.vcd.myid;
            sga.vcd.term += 1;

            // 同步给客户端
            msg.vcd.leader = sga.vcd.leader;
            msg.vcd.term = sga.vcd.term;

            // 广播给其他节点，通知其他节点leader已选出
            if let Some(sessions) = self.namespaces.get(&msg.namespace) {
                for sid in sessions {
                    // if sid == &msg.session_id {
                    //     continue;
                    // }
                    if let Some(addr) = self.sessions.get(sid) {
                        log::debug!(
                            "[{}] - [{}] - Server BROADCAST to session: {}...",
                            sga.vcd.myid,
                            sga.vcd.myid,
                            sid
                        );
                        let message = WsResponse::ok(
                            WsEvent::LEADER,
                            "Copy to Voter".to_owned(),
                            Some(msg.vcd.clone()),
                        )
                        .to_string();
                        addr.do_send(Message(message));
                    }
                }
            }
        }
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
        // notify all users in same cluster

        // register session with id
        let session_id = self.rng.gen::<usize>();
        self.sessions.insert(session_id, msg.addr);

        log::debug!(
            "[{}] - [{}] - Session created, session ID is {}",
            msg.myid,
            msg.voter_id,
            session_id
        );

        // self.session_map.insert(session_id, msg.id);
        self.namespaces
            .entry(msg.namespace)
            .or_default()
            .insert(session_id);

        // let _ = self.visitor_count.fetch_add(1, Ordering::SeqCst);
        // let count = self.visitor_count.load(Ordering::SeqCst);
        // self.send_message(&config::get_server_namespace(), &format!("Total visitors {count}"), 0);

        // send id back
        session_id
    }
}

// Handler for join message.
impl Handler<Join> for ChatServer {
    type Result = MessageResult<Join>;
    // 普通节点恢复
    // 原leader节点恢复后降级为普通节点

    fn handle(&mut self, mut msg: Join, _: &mut Context<Self>) -> Self::Result {
        MessageResult(msg)
    }
}

// Handler for Reset message.
impl Handler<Reset> for ChatServer {
    type Result = ();
    fn handle(&mut self, _msg: Reset, _: &mut Context<Self>) {
        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let mut sga = lock.lock();
        sga.vcd.poll = 1;
        sga.poll_to = None;
        sga.released = false;
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
impl Handler<Private> for ChatServer {
    type Result = ();
    fn handle(&mut self, msg: Private, _: &mut Context<Self>) {
        self.electing_leader(msg);
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
        log::debug!("Server Vote: `{:?}`", msg);
        log::debug!("Server SGA: `{:?}`", sga);

        // 有状态则不用选举
        match sga.status {
            shared::Status::LEADING | shared::Status::FOLLOWING => {
                msg.result = VoteResult::LEADER;
                log::debug!("[{}] - [{}] - join node...", sga.vcd.myid, msg.vcd.myid);
                // 同步给客户端
                msg.vcd.term = sga.vcd.poll;
                msg.vcd.leader = sga.vcd.leader;
                return MessageResult(msg);
            }
            _ => {}
        }

        if sga.vcd.poll == 0 {
            // 拒绝投票
            msg.result = VoteResult::ABANDON;
            log::debug!(
                "[{}] - [{}] - abandon candidate...",
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

        let poll = sga.vcd.poll;
        let voter_poll = msg.vcd.poll;

        // 是否投票转移
        let mut change = false;
        if term > voter_term {
            // 过期投票、无效投票
            // 投票人投票周期比本人小，忽略，忽略该节点的投票信息
            // 这种场景一般是由于离线较久后恢复。
            // 加入节点需要同步数据。
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

        if change {
            // 投票转移
            // 更新投票信息
            msg.vcd.poll_from.push(VFrom::new(myid, poll));
            // 更新自身的投票去向
            sga.poll_to = Some(VTo::new(msg.vcd.myid, poll));
            log::debug!(
                "[{}] - [{}] - Voting change actively, change votes {} to voter {}",
                sga.vcd.myid,
                msg.vcd.myid,
                poll,
                msg.vcd.myid
            );

            // 投票确认
            msg.vcd.poll += sga.vcd.poll;
            // 自身投票清空
            sga.vcd.poll = 0;
            // 全部票数投出去了，本节点已经变成投票人了(非候选人)
            sga.released = true;
            msg.result = VoteResult::CHANGE;
        } else {
            // 投票确认
            sga.vcd.poll += voter_poll;
            // 清空投票者的票数
            msg.vcd.poll = 0;
            // 投票节点数,
            // 如果存在多次投票,最后一次会覆盖前面的投票情况
            self.vote_from.insert(msg.vcd.myid, voter_poll);

            log::debug!(
                "[{}] - [{}] - Voting confirmed, received votes {}, total votes {}",
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

            msg.result = VoteResult::SUCCESS;

            // 选举leader
            // self.electing_header();
        }

        log::debug!(
            "[{}] - [{}] - Server Vote: SGA: {:?}",
            sga.vcd.myid,
            msg.vcd.myid,
            sga.clone()
        );
        cvar.notify_one();

        return MessageResult(msg);
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        if msg.closed {
            return;
        }

        let mut namespaces: Vec<String> = Vec::new();
        // let _ = self.visitor_count.fetch_sub(1, Ordering::SeqCst);

        // remove address
        if self.sessions.remove(&msg.id).is_some() {
            // remove session from all namespaces
            for (ns, sessions) in &mut self.namespaces {
                if sessions.remove(&msg.id) {
                    namespaces.push(ns.to_owned());
                }
                log::info!("{} - - Session {} removed from {}", msg.ip, msg.id, ns);
            }
        }
        // send message to other users
        for ns in namespaces {
            self.send_message(
                &ns,
                &format!("UID {} kicked out of the cluster [{}].", msg.id, ns),
                0,
            );

            // let count = self.visitor_count.load(Ordering::SeqCst);
            // self.send_message(&ns, &format!("Current Total visitors {count}"), 0);
        }
    }
}

/// Handler for Message message.
impl Handler<ClientMessage> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        self.send_message(&msg.cluster, msg.msg.as_str(), msg.id);
    }
}
