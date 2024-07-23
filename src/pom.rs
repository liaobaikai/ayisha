use std::{fs::{self, OpenOptions}, io::Write};

use serde::{Deserialize, Serialize};

use crate::config;

/// Persistent Object Manager
/// 
/// Data Persistent
/// 
/// Voter -> Server -> Data
/// Voter <- Server <- Data
/// 
/// 
/// ballot 投票
/// poll 投票，投票数
/// vote 投票，选票
/// candidate 候选人
/// elector 投票人，选民
/// voter 投票人，选民
/// canvass 游说；拉选票

/// 数据持久化对象
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoManager {
    pub c_state: State,
    pub v_state: State,
}

/// 候选人
/// 候选人的数据主要来源于投票人，自身不参与任何投票
pub struct Candidate {
    pub state: State
}

impl Candidate {

    pub fn new() -> Self {
        Candidate { 
            state: State::new() 
        }
    }
    
}

/// 投票人
/// 每次创建一个投票人，都回默认持有有效票数 1
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Voter {
    pub state: State
}

// 基本信息
/// <data_root>/<server_id>/state.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct State {
    // 服务器 ID
    pub id: usize,
    // 任期数
    pub term: usize,
    // 投票数
    pub poll: usize,
    // 事务次数，每同步一次数据后 +1
    pub tranx: usize,
    // 锁
    pub lock: bool,

}

impl State {
    pub fn new() -> Self {
        State {
            id: 0,
            term: 0,
            poll: 0,
            tranx: 0,
            lock: false
        } 
    }
}

impl Voter {

    pub fn new() -> Self {
        let mut state = State::new();
        // 初始票数
        state.poll = 1;
        // 事物 ID
        state.tranx = 1;
        // 选举周期
        state.term = 1;

        Voter { 
            state 
        }
    }
    
}

impl PoManager {

    pub const C_FILE_NAME: &'static str = "c_state.json";
    pub const V_FILE_NAME: &'static str = "v_state.json";
    // 
    pub fn new() -> Self {

        let c_path = config::get_server_root().join(PoManager::C_FILE_NAME);
        let v_path = config::get_server_root().join(PoManager::V_FILE_NAME);
        let server_id = config::get_server_id();

        // read to buffer
        let mut c_state: State = match fs::read_to_string(&c_path).and_then(|cont| {
            Ok(serde_json::from_str(&cont).unwrap())
        }) {
            Ok(s) => s,
            Err(e) => {
                log::debug!("[{server_id}] - [{server_id}] - JSON File {} parsing failed, cause: {}", c_path.display(), e);
                log::debug!("[{server_id}] - [{server_id}] - Build candidate data");
                Candidate::new().state
            }
        };
        c_state.id = server_id;

        let mut v_state: State = match fs::read_to_string(&v_path).and_then(|cont| {
            Ok(serde_json::from_str(&cont).unwrap())
        }) {
            Ok(s) => s,
            Err(e) => {
                log::debug!("[{server_id}] - [{server_id}] - JSON File {} parsing failed, cause: {}", v_path.display(), e);
                log::debug!("[{server_id}] - [{server_id}] - Build voter data");
                Voter::new().state
            }
        };
        v_state.id = config::get_server_id();

        PoManager { c_state, v_state }

    }

    // 持久化候选人的数据
    pub fn c_flush(&self) {

        let p = config::get_server_root().join(PoManager::C_FILE_NAME);
        let txt = serde_json::to_string(&self.c_state).unwrap();
        let id = self.c_state.id;

        match OpenOptions::new().write(true).create(true).truncate(true).open(&p) {
            Ok(mut f) => {
                f.write_all(&txt.as_bytes()).unwrap();
                f.flush().unwrap();
                log::debug!("[{id}] - [{id}] - Written candidate data, `{txt}`");
            },
            Err(e) => {
                log::error!("[{id}] - [{id}] - JSON File {} open failed, cause: {e}", &p.display());
            }
        }
    }

    // 持久化投票人的数据
    pub fn v_flush(&self) {
        let v_path = config::get_server_root().join(PoManager::V_FILE_NAME);
        let txt = serde_json::to_string(&self.v_state).unwrap();
        let id = self.v_state.id;

        match OpenOptions::new().write(true).create(true).truncate(true).open(&v_path) {
            Ok(mut f) => {
                f.write_all(&txt.as_bytes()).unwrap();
                f.flush().unwrap();
                log::debug!("[{id}] - [{id}] - Written voter data, `{txt}`");
            },
            Err(e) => {
                log::error!("[{id}] - [{id}] - JSON File {} open failed, cause: {e}", &v_path.display());
            }
        }
    }


    pub fn get_vdata(&mut self) -> State {
        
        let v_path = config::get_server_root().join(PoManager::V_FILE_NAME);
        let server_id = config::get_server_id();

        let mut v_state: State = match fs::read_to_string(&v_path).and_then(|cont| {
            Ok(serde_json::from_str(&cont).unwrap())
        }) {
            Ok(s) => s,
            Err(e) => {
                log::debug!("[{server_id}] - [{server_id}] - JSON File {} parsing failed, cause: {}", v_path.display(), e);
                log::debug!("[{server_id}] - [{server_id}] - Build voter data");
                Voter::new().state
            }
        };
        v_state.id = config::get_server_id();
        self.v_state = v_state.clone();
        v_state

    }

    pub fn get_cdata(&mut self) -> State {

        let c_path = config::get_server_root().join(PoManager::C_FILE_NAME);
        let server_id = config::get_server_id();

        // read to buffer
        let mut c_state: State = match fs::read_to_string(&c_path).and_then(|cont| {
            Ok(serde_json::from_str(&cont).unwrap())
        }) {
            Ok(s) => s,
            Err(e) => {
                log::debug!("[{server_id}] - [{server_id}] - JSON File {} parsing failed, cause: {}", c_path.display(), e);
                log::debug!("[{server_id}] - [{server_id}] - Build candidate data");
                Candidate::new().state
            }
        };
        c_state.id = server_id;
        self.c_state = c_state.clone();
        c_state

    }

}
