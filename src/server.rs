use std::{
    collections::{HashMap, HashSet}, net::IpAddr, sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    }
};
use actix::prelude::*;
use parking_lot::{Condvar, Mutex};
use rand::{rngs::ThreadRng, Rng};

use crate::{shared::{self, State}, ws::{self, WsEvent, WsRequest, WsResponse}};
// https://github.com/actix/examples/blob/master/websockets/chat/src/server.rs
// https://cloud.tencent.com/developer/article/1756850


// https://course.rs/advance/concurrency-with-threads/sync1.html
// use lazy_static::lazy_static;
// lazy_static! {
//     // pub static ref POLL_MUTEX: Mutex<usize> = Mutex::new(0);
//     // pub static ref TERM_MUTEX: Mutex<usize> = Mutex::new(0);
//     // pub static ref TRANX_MUTEX: Mutex<usize> = Mutex::new(0);

//     pub static ref GLOBAL_STATE_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, State>, Condvar)> = Arc::new((Mutex::new(State::new()), Condvar::new()));
//     // pub static ref GLOBAL_STATE_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
//     pub static ref GLOBAL_STATE_DATA: State = State::new();

//     // 当Voter 以及 server 端需要修改数据的时候，需要先获取锁，如果获取不到锁，则需要等待。
//     // 全局锁
//     // pub static ref GLOBAL_STATE_MUTEX: Mutex<usize> = Mutex::new(0);
// }

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
    pub ip: IpAddr
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

// 
// #[derive(Deserialize, Serialize, Debug)]
// pub struct ChatMessage {
//     pub typ: usize
// }

/// List of available clusters
// pub struct ListClusters;

// impl actix::Message for WsResponse {
//     type Result = Attribute;
// }

// pub struct JoinEvent;
// impl actix::Message for JoinEvent {
//     type Result = WsEvent;
// }

/// Join cluster, if room does not exists create new one.
#[derive(Message)]
#[rtype(Join)]
pub struct Join {
    pub session_id: usize,
    pub state: State
}

// 数据同步
#[derive(Message)]
#[rtype(Copy)]
pub struct Copy {
    pub session_id: usize,
    pub state: State
}

// 投票
#[derive(Message)]
#[rtype(Vote)]
pub struct Vote {
    pub session_id: usize,
    // 实际数据
    pub state: State,
    // 是否投票成功
    pub ok: bool
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
    poll_from: HashMap<usize, usize>,
    // 候选人状态信息
    // c_state: State,
    // 本地连接
    // local_session_list: Vec<usize>,
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
            poll_from: HashMap::new(),
            // c_state: State::new(),
            // local_session_list: Vec::new()
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

    fn update_sga_add_poll(&self, poll: usize){
        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let mut sga = lock.lock();
        sga.poll += poll;
        // 通知其他可以修改数据
        cvar.notify_all();
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

        // log::info!("{} - - [{}] connect to server", msg.ip, msg.id);

        // notify all users in same cluster

        // register session with id
        let session_id = self.rng.gen::<usize>();
        self.sessions.insert(session_id, msg.addr);

        log::debug!("[{}] - [{}] - Session created, session ID is {}", msg.myid, msg.voter_id, session_id);
        

        // self.session_map.insert(session_id, msg.id);
        self.namespaces.entry(msg.namespace).or_default().insert(session_id);

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
    // 0 => 本机刚恢复,投票周期比加入的节点要小,本机需要同步数据
    // 1 => 本机投票周期比加入节点的要大,加入节点需要同步数据
    // 2 => 投票周期一致
    
    // 普通节点恢复
    // 原leader节点恢复后降级为普通节点

    fn handle(&mut self, mut msg: Join, _: &mut Context<Self>) -> Self::Result {

        // 一票投给本机服务端
        // if self.s_attr.id == msg.attr.id && msg.attr.poll > 0 && self.polling_box.get(&msg.attr.id).is_none() {
        //     self.polling_box.insert(msg.attr.id, msg.attr.poll);
        //     self.s_attr.poll += msg.attr.poll;
        //     msg.attr.poll -= 1;
        //     log::debug!("Update Server Poll: {}, Client Poll: {}", self.s_attr.poll, msg.attr.poll);
        // }

        // // 连接上的节点数
        // self.attrs.insert(msg.attr.id, msg.attr.clone());

        // // 加入后判断是否需要进行数据同步
        
        // // 本机数据广播到其他会话,并排除自己
        // for (ns, _) in self.namespaces.clone() {
        //     let mut wsdata = msg.attr.clone();
        //     // 其他节点需要接收该数据
        //     wsdata.status = ws::WsDataStatus::RECV;
        //     // wsdata.tranx += 1;
        //     // 投票传递
        //     let resp = WsResponse::success(ws::WsEvent::BROADCAST, format!("Join and Copy data from {}", msg.attr.id), Some(wsdata));
        //     self.send_message(&ns, &resp.to_string(), msg.session_id);
        // }


        // 投票周期
        // let term = self.server_data.term;
        // // 刚加入的节点
        // let data = msg.data.clone();
        // if term > data.term {
        //     // 加入的节点投票周期比本节点小，忽略，忽略该节点的投票信息
        //     // 这种场景一般是由于离线较久后恢复。
        //     // 加入节点需要同步数据。
        //     msg.data.status = ws::WsDataStatus::SEND;
        //     // return MessageResult(msg);
        // } else if term < data.term {
        //     // 本机刚恢复，需向其他节点同步数据
        //     msg.data.status = ws::WsDataStatus::RECV;
        // } else {
        //     // 不需要同步数据
        //     msg.data.status = ws::WsDataStatus::NONTODO;
        // }


        MessageResult(msg)
        
    }
    
}

// 数据同步
impl Handler<Copy> for ChatServer {
    type Result = MessageResult<Copy>;
    // 0 => 本机刚恢复,投票周期比加入的节点要小,本机需要同步数据
    // 1 => 本机投票周期比加入节点的要大,加入节点需要同步数据
    // 2 => 投票周期一致,未进行数据同步

    fn handle(&mut self, msg: Copy, _: &mut Context<Self>) -> Self::Result {

        // if let Ok(mut mg) = POLL_MUTEX.try_lock() {
        //     *mg += msg.state.poll;
        // }
        // let &(ref lock, ref cvar) = &*self.pair.clone();

        // 从 Voter 同步到 Server 端
        // self.c_state = msg.state.clone();

        // log::debug!("[{}] - [{}] - Copy data from voter, state: `{}`", self.pom.c_state.id, msg.state.id, serde_json::to_string(&self.c_state).unwrap());

        MessageResult(msg)
    }
    
}



// leader竞选
// 1.term大的直接胜出
// 2.term相同，事务id大的胜出
// 3.事务id相同，服务器id大的胜出
impl Handler<Vote> for ChatServer {
    type Result = MessageResult<Vote>;

    fn handle(&mut self, mut msg: Vote, _: &mut Context<Self>) -> Self::Result {
        // 3
        // let node_count = config::get_nodes().len();
        // // 加入的数量
        // let join_node_count = self.attrs.keys().len();
        // // 加入计算者本身
        // // join_node_count + 1;
        // let mut min_node_count = node_count / 2;
        // if min_node_count > 1 {
        //     min_node_count -= 1;
        // }

        // // 3个节点，至少超过一半存活
        // if !(join_node_count > min_node_count) {
        //     // 需要等待其他节点
        //     log::info!("Non election conditions, insufficient voting node, waitting for other node join...");
        //     return MessageResult(msg);
        // }

        // // 可以开始计算
        // log::info!("Meet election requirements...");

        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        let mut sga = lock.lock();
        // 候选人：投票周期
        let term = sga.term;
        // 投票人：投票周期
        let voter_term = msg.state.term;
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
            log::debug!("[{}] - [{}] - Voting change actively, candidate term is {}, voter term is {}", sga.myid, msg.state.myid, term, voter_term);
        } else {
            // 投票周期一致
            // 判断tranx，tranx越大，则越新
            
            // 事务数
            let tranx = sga.tranx;
            let voter_tranx = msg.state.tranx;

            // 比较事务数，事务数越大，则代表该节点的数据越新
            if tranx > voter_tranx {
                // 本机作为leader
            } else if tranx < voter_tranx {
                // 候选人失去候选机会，投票失败，票数转移
                change = true;
                log::debug!("[{}] - [{}] - Voting change actively, candidate tranx is {}, voter tranx is {}", sga.myid, msg.state.myid, tranx, voter_tranx);
            } else {
                // 比对 ID，ID 越大，就投票给大的
                let id = sga.myid;
                let voter_id = msg.state.myid;
                if id < voter_id {
                    // 候选人失去候选机会，投票失败，票数转移
                    change = true;
                    log::debug!("[{}] - [{}] - Voting change actively, candidate id is {}, voter id is {}", sga.myid, msg.state.myid, id, voter_id);
                }
            }
        }

        if change {
            // 投票转移
            msg.state.vote_from = Some(shared::Vote { id: sga.myid, poll: sga.poll });
            sga.vote_to = Some(msg.state.myid);
            log::debug!("[{}] - [{}] - Voting change actively, change votes {} to voter {}", sga.myid, msg.state.myid, sga.poll, msg.state.myid);
            // 自身投票清空
            sga.poll = 0;
        } else {
            // 投票确认
            sga.poll += msg.state.poll;
            log::debug!("[{}] - [{}] - Voting confirmed, received votes {}, total votes {}", sga.myid, msg.state.myid, msg.state.poll, sga.poll);
            // 清空投票者的票数
            msg.state.poll = 0;
            sga.vote_from(&msg.state.myid);
            msg.ok = true;
        }

        println!("Server Vote: SGA: {:?}", sga.clone());
        cvar.notify_one();
        
        // 刷盘
        // self.pom.c_flush();

        // 数据同步到Voter
        // for sid in self.local_session_list.iter() {
        //     if let Some(addr) = self.sessions.get(sid) {
        //         let message = WsResponse::ok(WsEvent::COPY, "Copy to Voter".to_owned(), Some(self.c_state.clone())).to_string();
        //         addr.do_send(Message(message));
        //     }
        // }
        
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
            self.send_message(&ns, &format!("UID {} kicked out of the cluster [{}].", msg.id, ns), 0);

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

/*
/// Handler for `ListRooms` message.
impl Handler<ListClusters> for ChatServer {
    type Result = MessageResult<ListClusters>;

    fn handle(&mut self, _: ListClusters, _: &mut Context<Self>) -> Self::Result {
        let mut clusters = Vec::new();

        for key in self.clusters.keys() {
            clusters.push(key.to_owned())
        }

        MessageResult(clusters)
    }
} */

/*
/// Join cluster, send disconnect message to old cluster
/// send join message to new cluster
impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join { id, name } = msg;
        let mut clusters = Vec::new();

        // remove session from all clusters
        for (n, sessions) in &mut self.clusters {
            if sessions.remove(&id) {
                clusters.push(n.to_owned());
            }
        }
        // send message to other users
        for room in clusters {
            self.send_message(&room, "Someone disconnected", 0);
        }

        self.clusters.entry(name.clone()).or_default().insert(id);

        self.send_message(&name, "Someone connected", id);
    }
}
 */