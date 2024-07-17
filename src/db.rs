use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlite::{Connection, ConnectionThreadSafe, State, Value};

use crate::config;

#[derive(Debug)]
pub struct UserAuth {

    // if null, db return 0
    pub id: usize,
    pub app_key: String,
    pub app_secret: String,
    pub host: String,
    pub expired_datetime: Option<NaiveDateTime>

}

#[derive(Debug, Clone, Deserialize, Serialize)]
// Cache DB Node
pub struct Node {
    // Event
    pub event: String,

    // if null, db return 0
    pub id: usize,
    // 是否为leader
    pub header: usize,
    pub host: String,
    // 端口
    pub port: u16,
    // 权重
    pub weight: usize,
    // 任期数
    pub term: usize,
    // 投票数
    pub poll: usize,
    // 事务次数，每同步一次数据后 +1
    pub tranx: usize,
    // 上一次心跳时间：时间戳
    pub last_hb_time: i64

}


impl UserAuth {
    const CREATE_TABLE_DDL: &'static str = "
        CREATE TABLE IF NOT EXISTS user_auth (
            id INTEGER PRIMARY KEY AUTOINCREMENT, 
            app_key varchar(256), 
            app_secret varchar(256), 
            host varchar(256),
            expired_datetime varchar(32)
        )";
    const SELECT_SQL: &'static str = "SELECT * FROM user_auth WHERE app_key = :app_key and app_secret = :app_secret";

}

impl Node {
    const CREATE_TABLE_DDL: &'static str = "
        CREATE TABLE IF NOT EXISTS node (
            id INTEGER PRIMARY KEY AUTOINCREMENT, 
            header INTEGER, 
            host varchar(256), 
            port INTEGER, 
            weight INTEGER, 
            term INTEGER, 
            poll INTEGER, 
            tranx INTEGER,
            last_hb_time INTEGER,
            event varchar(32) 
        )";
    const REPLACE_SQL: &'static str = "REPLACE INTO node (id, header, host, port, weight, term, poll, tranx, last_hb_time, event) VALUES (:id, :header, :host, :port, :weight, :term, :poll, :tranx, :last_hb_time, :event)";
    const SELECT_SQL: &'static str = "select id, header, host, port, weight, term, poll, tranx, last_hb_time, event from node where id = :id";

    // const EVENT_JOIN: &'static str = "event/join";
    // const EVENT_COPY: &'static str = "event/copy";
    // const EVENT_VOTE: &'static str = "event/vote";
}

pub struct DB {
    conn: ConnectionThreadSafe,
}

impl DB {

    pub fn new() -> Self {

        let s_path = config::get_server_sqlite();

        let db_conn = match Connection::open_thread_safe(&s_path) {
            Ok(conn) => conn,
            Err(e) => {
                panic!("database {} cannot be opened, cause: {}", s_path, e);
            }
        };

        db_conn.execute(UserAuth::CREATE_TABLE_DDL).unwrap();
        db_conn.execute(Node::CREATE_TABLE_DDL).unwrap();

        DB { conn: db_conn }

    }

    
    // 查询节点信息
    pub fn query_node(&self, id: &str) -> Option<Node> {

        match self.conn.prepare(Node::SELECT_SQL) {
            Ok(mut stmt) => {
                stmt.bind((":id", id)).unwrap();

                let mut op = None;
                if let Ok(State::Row) = stmt.next() {
                    op = Some(Node { 
                        id: stmt.read::<i64, _>(stmt.column_name(0).unwrap()).unwrap() as usize, 
                        header: stmt.read::<i64, _>(stmt.column_name(1).unwrap()).unwrap() as usize, 
                        host: stmt.read::<String, _>(stmt.column_name(2).unwrap()).unwrap(), 
                        port: stmt.read::<i64, _>(stmt.column_name(3).unwrap()).unwrap() as u16, 
                        weight: stmt.read::<i64, _>(stmt.column_name(4).unwrap()).unwrap() as usize, 
                        term: stmt.read::<i64, _>(stmt.column_name(5).unwrap()).unwrap() as usize, 
                        poll: stmt.read::<i64, _>(stmt.column_name(6).unwrap()).unwrap() as usize, 
                        tranx: stmt.read::<i64, _>(stmt.column_name(7).unwrap()).unwrap() as usize,
                        last_hb_time: stmt.read::<i64, _>(stmt.column_name(8).unwrap()).unwrap(),
                        event: stmt.read::<String, _>(stmt.column_name(9).unwrap()).unwrap(), 
                    });
                }

                op
            },
            Err(e) => {
                panic!("query_node prepare failed, cause: {}", e);
            }
        }
    }

    // 查询用户权限
    pub fn query_userauth(self, app_key: &str, app_secret: &str) -> Vec<UserAuth>{

        match self.conn.prepare(UserAuth::SELECT_SQL) {
            Ok(mut stmt) => {
                stmt.bind((":app_key", app_key)).unwrap();
                stmt.bind((":app_secret", app_secret)).unwrap();

                // log::debug!("Prepare-SQL: `{}`", UserAuth::SELECT_SQL);
                // log::debug!("bind: {}, {}", app_key, app_secret);

                let mut rows = Vec::new();
                while let Ok(State::Row) = stmt.next() {
                    /*
                    implement!(Vec<u8>, Binary);
                    implement!(&[u8], Binary);
                    implement!(f64, Float);
                    implement!(i64, Integer);
                    implement!(String, String);
                    implement!(&str, String);
                    implement!((), Null); */

                    let mut value = stmt.read::<String, _>(stmt.column_name(4).unwrap()).unwrap();
                    let datetime;
                    if value.is_empty() {
                        value = String::from("9999-12-31 01:01:01");
                    }
                    datetime = match NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S") {
                        Ok(datetime) => Some(datetime),
                        Err(e) => {
                            log::error!("datetime parsing failed, cause: {}", e);
                            None
                        }
                    };

                    rows.push(UserAuth {
                        id: stmt.read::<i64, _>(stmt.column_name(0).unwrap()).unwrap() as usize, 
                        app_key: stmt.read::<String, _>(stmt.column_name(1).unwrap()).unwrap(), 
                        app_secret: stmt.read::<String, _>(stmt.column_name(2).unwrap()).unwrap(), 
                        host: stmt.read::<String, _>(stmt.column_name(3).unwrap()).unwrap(),
                        expired_datetime: datetime,
                    });
                }
                rows
            },
            Err(e) => {
                panic!("query_userauth prepare failed, cause: {}", e);
            }
        }

        
    }


}


impl Node {

    pub fn new() -> Self {
        Node { 
            event: String::from(""),
            id: config::get_server_id(), 
            header: 0, 
            host: String::from("127.0.0.1"), 
            port: config::get_server_port(), 
            weight: config::get_server_weight(), 
            term: 0, 
            poll: 0, 
            tranx: 0,
            last_hb_time: 0
        }
    }


    // 数据刷新到数据库中
    pub fn save_node(&self, db: &DB) {

        // self.conn.prepare("REPLACE INTO nodes VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)").unwrap().into_iter().bind((1, node.id.into())).unwrap();
// (
    // id INTEGER PRIMARY KEY AUTOINCREMENT, 
    // header INTEGER, 
    // host varchar(256), 
    // port INTEGER, 
    // weight INTEGER, 
    // poll INTEGER, 
    // tranx INTEGER
        match db.conn.prepare(Node::REPLACE_SQL) {
            Ok(mut stmt) => {
                stmt.bind((":id", Value::Integer(self.id as i64))).unwrap();
                stmt.bind((":header", Value::Integer(self.header as i64))).unwrap();
                stmt.bind((":host", Value::String(self.host.clone()))).unwrap();
                stmt.bind((":port", Value::Integer(self.port as i64))).unwrap();
                stmt.bind((":weight", Value::Integer(self.weight as i64))).unwrap();
                stmt.bind((":poll", Value::Integer(self.poll as i64))).unwrap();
                stmt.bind((":tranx", Value::Integer(self.tranx as i64))).unwrap();
                stmt.bind((":last_hb_time", Value::Integer(self.last_hb_time))).unwrap();

                stmt.next().unwrap();

            },
            Err(e) => {
                panic!("save_node prepare failed, cause: {}", e);
            }
        }

    }

    


}