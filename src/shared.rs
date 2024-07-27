use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::config;

lazy_static! {
    pub static ref SHARE_GLOBAL_AREA_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, GlobalArea>, Condvar)> = Arc::new((Mutex::new(GlobalArea::new()), Condvar::new()));
}

pub const NON_LEADER: usize = 0;

#[derive(Debug, Clone)]
pub struct GlobalArea {
    // 投票数据
    pub vcd: VCData,
    // 票数去向
    pub poll_to: Option<VTo>,
    // 是否放弃成为候选人
    pub released: bool,
    // 状态：启动后默认
    pub status: Status
}

#[derive(Debug, Clone)]
pub enum Status {
    LOOKING,
    FOLLOWING,
    LEADING
}

impl GlobalArea {
    
    pub fn new() -> Self {
        GlobalArea{ 
            vcd: VCData::new(1),
            poll_to: None,
            released: false,
            status: Status::LOOKING
        } 
    }

    pub fn poll_from(&mut self, v: VFrom) -> usize {
        if !self.is_poll_from(&v.from_id) {
            self.vcd.poll_from.push(v.clone());
            return v.poll
        }
        0
    }

    pub fn is_poll_from(&mut self, id: &usize) -> bool {
        for v in self.vcd.poll_from.iter() {
            if v.from_id == id.to_owned() {
                return true
            }
        }
        false
    }

    pub fn is_poll_to(&mut self, id: &usize) -> bool {
        if let Some(v) = &self.poll_to {
            if v.to_id == id.to_owned() {
                return true
            }
        }
        false
    }

    pub fn get_vcd(&self) -> VCData {
        return self.vcd.clone();
    }

}

// 基本信息
// Vote Core Data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VCData {
    // leader ID
    pub leader: usize,
    pub myid: usize,
    // 任期数
    pub term: usize,
    // 投票数
    pub poll: usize,
    // 事务次数，每同步一次数据后 +1
    pub tranx: usize,
    // 支持者
    pub poll_from: Vec<VFrom>

}

impl VCData {
    pub fn new(poll: usize) -> Self {

        VCData {
            leader: NON_LEADER,
            myid: config::get_server_id(),
            term: 0,
            poll,
            tranx: 0,
            poll_from: Vec::new()
        } 
    }
}


// Vote From
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VFrom {
    // 投票来源 id
    pub from_id: usize,
    // 投出票数
    pub poll: usize,
}

impl VFrom {
    pub fn new(from_id: usize, poll: usize) -> Self {
        VFrom {
            from_id,
            poll
        }
    }
}


// Vote To
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VTo {
    // 投票去向 id
    pub to_id: usize,
    // 投出票数
    pub poll: usize,
}

impl VTo {
    pub fn new(to_id: usize, poll: usize) -> Self {
        VTo {
            to_id,
            poll
        }
    }
}


