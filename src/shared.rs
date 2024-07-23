use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

use lazy_static::lazy_static;
lazy_static! {
    pub static ref SHARE_GLOBAL_AREA_MUTEX_PAIR: Arc<(parking_lot::lock_api::Mutex<parking_lot::RawMutex, ShareGlobalArea>, Condvar)> = Arc::new((Mutex::new(ShareGlobalArea::new()), Condvar::new()));
}


#[derive(Clone)]
pub struct ShareGlobalArea {

    // 票数
    pub poll: usize,
    // 事务
    pub tranx: usize,
    // 投票周期
    pub term: usize,

    pub lock: Arc<parking_lot::lock_api::Mutex<parking_lot::RawMutex, bool>>

}

impl ShareGlobalArea {
    
    pub fn new() -> Self {
        ShareGlobalArea{ 
            poll: 0,
            tranx: 0,
            term: 0,
            lock: Arc::new(Mutex::new(false))
        } 
    }
}