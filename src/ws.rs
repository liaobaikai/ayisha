use bytestring::ByteString;
use serde::{Deserialize, Serialize};

use crate::shared::State;


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsRequest {

    // 事件
    pub event: WsEvent,

    // json数据
    pub state: State

}


// #[derive(Debug, Clone, Deserialize, Serialize)]
// pub struct Profile {

//     // 事件
//     pub event: WsEvent,

//     // json数据
//     pub attr: Attribute

// }

impl WsRequest {
    pub fn from_str<'a>(s: &'a str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_str(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn to_bytestr(&self) -> ByteString {
        let buf: ByteString = serde_json::to_string(self).unwrap().to_owned().into();
        buf
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsResponse {
    // 事件
    pub event: WsEvent,
    // 状态码
    pub status_code: WsStatusCode,
    // 消息
    pub mail_box: String,
    // json数据
    pub state: Option<State>,

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WsStatusCode {
    // 处理成功
    SUCCESS,
    // 指的是处理失败
    FAILED,     
    // 指的是处理过程有发生异常
    ERROR, 
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WsEvent {
    UNKNOWN,
    JOIN,
    COPY,
    // 投票
    VOTE,
    // 改票
    CHANGED,
    // 广播数据：传递数据到其他节点
    BROADCAST
}

// #[derive(Debug, Clone, Deserialize, Serialize)]
// pub struct Attribute {
//     pub id: usize,
//     // 任期数
//     pub term: usize,
//     // 投票数
//     pub poll: usize,
//     // 事务次数，每同步一次数据后 +1
//     pub tranx: usize,
//     // 数据同步情况
//     pub status: WsDataStatus

// }

// 0 => 本机刚恢复,投票周期比加入的节点要小,本机需要同步数据
// 1 => 本机投票周期比加入节点的要大,加入节点需要同步数据
// 2 => 投票周期一致
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WsDataStatus {
    // 本机需同步数据
    RECV,
    // 其他节点需要同步数据
    SEND,
    // 投票周期一致
    NONTODO
}

impl WsResponse {

    pub fn from_str<'a>(s: &'a str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn ok(event: WsEvent, mail_box: String, state: Option<State>) -> Self {
        WsResponse { event, status_code: WsStatusCode::SUCCESS, mail_box, state }
    }

    pub fn failed(event: WsEvent, mail_box: String, state: Option<State>) -> Self {
        WsResponse { event, status_code: WsStatusCode::FAILED, mail_box, state }
    }

    pub fn error(event: WsEvent, mail_box: String, state: Option<State>) -> Self {
        WsResponse { event, status_code: WsStatusCode::ERROR, mail_box, state }
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }

    pub fn to_bytestr(&self) -> ByteString {
        let buf: ByteString = serde_json::to_string(self).unwrap().to_owned().into();
        buf
    }

    // pub fn new() -> Self {
    //     WsResponse { event: WsEvent::UNKNOWN, status_code: WsStatusCode::SUCCESS, mail_box: String::new(), state: None }
    // }

}

// impl Attribute {
//     pub fn new(node: Node) -> Self {
//         Attribute { id: node.id, term: node.term, poll: node.poll, tranx: node.tranx, status: WsDataStatus::RECV }
//     }

// }