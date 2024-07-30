use std::{collections::HashMap, sync::Arc};

use chrono::Duration;
use parking_lot::{Condvar, Mutex};

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::config;

lazy_static! {
    pub static ref SHARE_GLOBAL_AREA_MUTEX_PAIR: Arc<(
        parking_lot::lock_api::Mutex<parking_lot::RawMutex, GlobalArea>,
        Condvar
    )> = Arc::new((Mutex::new(GlobalArea::new()), Condvar::new()));
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
    pub status: Status,
    // 如果leader选举一直等不到过半的节点存活，则超过这个时间，就退出，避免脑裂
    pub discovery_wait_timeout: Duration,
    // 连接失败的节点
    pub hb_failed: HashMap<usize, usize>,
}

#[derive(Debug, Clone)]
pub enum Status {
    Looking,
    Following,
    Leading,
}

impl GlobalArea {
    pub fn new() -> Self {
        GlobalArea {
            vcd: VCData::new(1),
            poll_to: None,
            released: false,
            status: Status::Looking,
            discovery_wait_timeout: Duration::seconds(
                config::get_server_discovery_wait_timeout() as i64
            ),
            hb_failed: HashMap::new()
        }
    }

    /// 重置
    pub fn reset_with_vote(&mut self) {
        self.vcd.reset_with_vote();
        self.poll_to = None;
        self.released = false;
        self.status = Status::Looking;
    }

    /// 降级为following
    pub fn to_following(&mut self) {
        self.vcd.reset_with_vote();
        self.poll_to = None;
        self.released = false;
        self.status = Status::Looking;
    }

    pub fn poll_from(&mut self, v: VFrom) -> usize {
        if !self.is_poll_from(&v.from_id) {
            self.vcd.poll_from.push(v.clone());
            return v.poll;
        }
        0
    }

    pub fn is_poll_from(&mut self, id: &usize) -> bool {
        for v in self.vcd.poll_from.iter() {
            if v.from_id == id.to_owned() {
                return true;
            }
        }
        false
    }

    pub fn is_poll_to(&mut self, id: &usize) -> bool {
        if let Some(v) = &self.poll_to {
            if v.to_id == id.to_owned() {
                return true;
            }
        }
        false
    }

    pub fn get_vcd(&self) -> VCData {
        return self.vcd.clone();
    }

    pub fn changed_poll(&mut self, id: usize) {
        if !self.vcd.poll_changed.contains(&id) {
            self.vcd.poll_changed.push(id);
        }
    }

    pub fn sync_leader(&mut self, leader: usize, term: usize, status: Status) {
        self.vcd.sync_leader(leader, term);
        self.status = status;
    }

    pub fn is_leader(&self, id: usize) -> bool {
        self.vcd.leader == id
    }

    pub fn is_not_looking(&self) -> bool {
        match self.status {
            Status::Leading | Status::Following => true,
            _ => false,
        }
    }

    // 添加心跳失败的节点
    // pub fn hb_failed(&mut self, to: usize, from: usize) {
    //     self.vcd.hb_failed(to, from);
    // }

    // pub fn remove_hb_failed(&mut self, to: usize) {
    //     self.vcd.hb_failed.remove(&to);
    // }

    pub fn fmt(&self) -> String {
        format!(
            "{{ status = \"{:?}\", released = {}, poll_to = {}, vcd = {} }}",
            self.status,
            self.released,
            serde_json::to_string(&self.poll_to).unwrap(),
            serde_json::to_string(&self.vcd).unwrap()
        )
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
    pub poll_from: Vec<VFrom>,
    // 投票转移
    pub poll_changed: Vec<usize>,
    // 心跳失败异常节点: from, to
    // 如果出现节点孤岛的时候，应该把leader降级为follower
    // 网络出现异常，导致全部节点无法互相连接时
    // pub hb_failed: HashMap<usize, usize>
}

impl VCData {
    pub fn new(poll: usize) -> Self {
        VCData {
            leader: NON_LEADER,
            myid: config::get_server_id(),
            term: 0,
            poll,
            tranx: 0,
            poll_from: Vec::new(),
            poll_changed: Vec::new(),
            // hb_failed: HashMap::new(),
        }
    }

    pub fn sync_leader(&mut self, leader: usize, term: usize) {
        self.leader = leader;
        // 同步投票周期
        self.term = term;
    }

    pub fn reset_with_vote(&mut self) {
        self.leader = NON_LEADER;
        self.poll = 1;
        self.poll_from = Vec::new();
        self.poll_changed = Vec::new();
    }

    // 添加心跳失败的节点
    // pub fn hb_failed(&mut self, to: usize, from: usize) {
    //     self.hb_failed.insert(to, from);
    // }

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
        VFrom { from_id, poll }
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
        VTo { to_id, poll }
    }
}
