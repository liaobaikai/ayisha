use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::config;
lazy_static! {
    pub static ref SHARE_GLOBAL_AREA_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, ShareGlobalArea>, Condvar)> = Arc::new((Mutex::new(ShareGlobalArea::new()), Condvar::new()));
}


#[derive(Debug, Clone)]
pub struct ShareGlobalArea {
    // 候选人ID
    pub myid: usize,
    // 票数
    pub poll: usize,
    // 事务
    pub tranx: usize,
    // 投票周期
    pub term: usize,
    // 投票来源
    pub poll_from: Vec<Vote>,
    // 我投票给那个ID
    pub poll_to: Option<Vote>
}

impl ShareGlobalArea {
    
    pub fn new() -> Self {
        ShareGlobalArea{ 
            myid: config::get_server_id(), 
            poll: 1,
            tranx: 0,
            term: 0,
            poll_from: Vec::new(),
            poll_to: None
        } 
    }

    // false: 已经投票过，无需重复投票
    // true: 投票成功
    pub fn poll_from(&mut self, v: Vote) {
        if self.is_poll_from(&v.from_id).is_none() {
            self.poll_from.push(v);
        }
    }

    pub fn is_poll_from(&mut self, id: &usize) -> Option<Vote> {
        for v in self.poll_from.iter() {
            if v.from_id == id.to_owned() {
                return Some(v.to_owned());
            }
        }
        None
    }

    pub fn is_poll_to(&mut self, id: &usize) -> Option<Vote> {
        if let Some(v) = &self.poll_to {
            if v.from_id == id.to_owned() {
                return Some(v.to_owned())
            }
        }
        None
    }

}


// 基本信息
/// <data_root>/<server_id>/state.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct State {
    pub myid: usize,
    // 任期数
    pub term: usize,
    // 投票数
    pub poll: usize,
    // 事务次数，每同步一次数据后 +1
    pub tranx: usize,
    // 支持者
    pub poll_from: Vec<Vote>

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vote {
    // 目标 id
    // pub id: usize,
    // 投票来源 id
    pub from_id: usize,
    // 投出票数
    pub poll: usize,
}

impl State {
    pub fn new(sga: &ShareGlobalArea) -> Self {

        State {
            myid: sga.myid,
            term: sga.term,
            poll: sga.poll,
            tranx: sga.tranx,
            poll_from: sga.poll_from.clone()
        } 
    }
}

