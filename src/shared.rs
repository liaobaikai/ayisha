use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

use lazy_static::lazy_static;

use crate::config;
lazy_static! {
    pub static ref SHARE_GLOBAL_AREA_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, ShareGlobalArea>, Condvar)> = Arc::new((Mutex::new(ShareGlobalArea::new()), Condvar::new()));
}


#[derive(Clone)]
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
    pub voters: Vec<usize>
}

impl ShareGlobalArea {
    
    pub fn new() -> Self {
        ShareGlobalArea{ 
            myid: config::get_server_id(), 
            poll: 0,
            tranx: 0,
            term: 0,
            voters: Vec::new()
        } 
    }

    // false: 已经投票过，无需重复投票
    // true: 投票成功
    pub fn vote(&mut self, voter_id: usize) -> bool {

        if self.voters.contains(&voter_id) {
            return false;
        }
        self.voters.push(voter_id);
        self.voters.contains(&voter_id)
    }
}