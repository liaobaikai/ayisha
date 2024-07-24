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
    // 投票给我的有哪些voter ID
    pub vote_from: Vec<usize>,
    // 我投票给那个ID
    pub vote_to: Option<usize>
}

impl ShareGlobalArea {
    
    pub fn new() -> Self {
        ShareGlobalArea{ 
            myid: config::get_server_id(), 
            poll: 1,
            tranx: 0,
            term: 0,
            vote_from: Vec::new(),
            vote_to: None
        } 
    }

    // false: 已经投票过，无需重复投票
    // true: 投票成功
    pub fn vote_from(&mut self, id: &usize) {
        self.vote_from.push(id.to_owned());
    }

    pub fn is_vote_from(&mut self, id: &usize) -> bool{
        self.vote_from.contains(id)
    }

    pub fn is_vote_to(&mut self, id: &usize) -> bool{
        if let Some(_id) = &self.vote_to {
            _id == id
        } else {
            false
        }
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
    pub vote_from: Option<Vote>

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vote {
    // 支持者ID
    pub id: usize,
    // 投票数
    pub poll: usize,
}

impl State {
    pub fn new(sga: &ShareGlobalArea) -> Self {
        State {
            myid: sga.myid,
            term: sga.term,
            poll: sga.poll,
            tranx: sga.tranx,
            vote_from: None
        } 
    }
}

