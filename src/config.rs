use std::collections::HashMap;
use std::{env, fs};
use std::path::{Path, PathBuf};
use std::{fs::File, io};
use std::io::Error;

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

lazy_static! {
    pub static ref APP_CONFIG: AppConfig = get_appconfig();
    // pub static ref DB_CONN: Connection = get_db_connection();
}

pub static DEFAULT_PORT: u16 = 6969;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig{
    // 服务端
    pub server: Option<AppConfigServer>,
    // 客户端
    pub client: Option<AppConfigClient>,
    // 心跳信息      
    pub heartbeat: Option<AppConfigHeartbeat>,
    // 集群服务节点
    pub node: Option<Vec<AppConfigNode>>
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfigServer {

    // ID：默认为1
    pub server_id: Option<usize>,  
    // 启动端口：默认为6969
    pub port: Option<u16>,        
    // 启动线程数：默认为2   
    pub worker: Option<usize>,       
    // 命名空间：默认为 defaults
    pub namespaces: Option<Vec<String>>,
    // 默认数据库：默认为 ./${id}/app.db
    // pub sqlite: Option<String>,                     
    // 权重：默认为1
    pub weight: Option<usize>,
    // 数据文件目录
    pub data_root: Option<String>,

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfigClient {
    // 是否跟随server启动，默认为false
    pub singleton: Option<bool>,

    // APP_KEY
    pub app_key: Option<String>,

    // APP_SECRET
    pub app_secret: Option<String>,

    // 连接丢失的情况下 重新尝试连接主服务器之前睡眠的秒数
    pub connect_retry: Option<usize>,

    // 命名空间：默认为 defaults
    pub namespace: Option<String>,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfigHeartbeat {
    // 心跳间隔，秒
    pub interval: Option<usize>,
    // 心跳超时，秒
    pub timeout: Option<usize>
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfigNode {
    // ID，默认为1
    pub id: usize,
    // 服务节点，默认127.0.0.1
    pub host: Option<String>,
    // 服务端口，默认6969
    pub port: Option<u16>
}

/// Host Based Authorization - Ident
#[derive(Debug, Clone, Deserialize)]
pub struct HostBasedAuth {
    pub map: HashMap<String, Vec<Ident>>,
    
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ident {
    pub user: String,
    pub app_key: String,
    pub app_secret: String,
    pub namespace: String,
}

impl HostBasedAuth {
    
    pub fn new() -> Self {

        let path = "./hba_ident.toml";

        let toml_text = fs::read_to_string(path).unwrap();
        let hba: HostBasedAuth = match toml::from_str(&toml_text) {
            Ok(p) => p,
            Err(e) => {
                panic!("<ParseTOML> File {} cannot be parsed, CAUSE:\n{}", path, e);
            }
        };
        hba
    }

    pub fn match_ident(&self, app_key: &str, app_secret: &str) -> HashMap<String, Ident> {

        let mut ret = HashMap::new();

        for (host, idents) in self.map.iter() {
            for ident in idents {
                if ident.app_key.eq_ignore_ascii_case(app_key) && ident.app_secret.eq_ignore_ascii_case(app_secret) {
                    ret.insert(host.to_owned(), ident.to_owned());
                }
            }
        }

        ret
    }
}


#[derive(Debug, StructOpt)]
pub struct Opt {

    #[structopt(short, long)]
    pub debug: bool,

    #[structopt(short="-i", long, parse(try_from_str=parse_file_path))]
    pub defaults_file: String,

}

// SERVER
pub fn get_server_id() -> usize {
    APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.server_id }).unwrap_or(1)
}

pub fn get_server_port() -> u16 {
    APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.port }).unwrap_or(DEFAULT_PORT)
}

pub fn get_server_worker() -> usize {
    APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.worker }).unwrap_or(2)
}

pub fn get_server_namespaces() -> Vec<String> {
    let mut default = Vec::new();
    default.push(String::from("default"));
    APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.namespaces }).unwrap_or(default)
}

// pub fn get_server_sqlite() -> String {
//     let path = APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.sqlite }).unwrap_or(format!("{}/{}/ayisha.db", get_server_data_root(), get_server_id()));
//     let parent = Path::new(&path).parent().unwrap();
//     let _ = fs::create_dir_all(parent).unwrap();
//     path
// }

pub fn get_server_weight() -> usize {
    APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.weight }).unwrap_or(1)
}

pub fn get_server_data_root() -> PathBuf {
    let pwd = env::current_dir().unwrap();
    let default_data_root = pwd.join(format!(".{}", env!("CARGO_PKG_NAME")));
    let data_root = APP_CONFIG.server.clone().and_then(|s: AppConfigServer| { s.data_root }).unwrap_or(default_data_root.display().to_string());

    let path = Path::new(&data_root).to_path_buf();
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn get_server_root() -> PathBuf {
    let path = get_server_data_root().join(format!("{}", get_server_id()));
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn get_nodes() -> Vec<AppConfigNode> {
    let nodes = APP_CONFIG.node.clone().unwrap_or(Vec::new());
    if nodes.len() < 3 {
        panic!("[[node]] At least 3 nodes");
    }
    nodes
}




// pub fn get_db_connection() -> Connection {
//     let p = get_server_sqlite();
//     log::debug!("Open sqlite db: {}", p);
//     let conn = match Connection::open(&p) {
//         Ok(conn) => conn,
//         Err(e) => {
//             panic!("<sqlite> Database {} cannot be opened, CAUSE: {}", p, e);
//         }
//     };
//     for sql in INIT_SQL_LIST {
//         let _ = conn.execute(sql, ());
//     }
//     conn
// }


// CLIENT 
pub fn get_client_singleton() -> bool {
    APP_CONFIG.client.clone().and_then(|c| { c.singleton }).unwrap_or(false)
}

pub fn get_client_app_key() -> String {
    APP_CONFIG.client.clone().and_then(|c| { c.app_key }).unwrap_or(String::new())
}

pub fn get_client_app_secret() -> String {
    APP_CONFIG.client.clone().and_then(|c| { c.app_secret }).unwrap_or(String::new())
}

pub fn get_client_connect_retry() -> usize {
    APP_CONFIG.client.clone().and_then(|c| { c.connect_retry }).unwrap_or(60)
}

pub fn get_client_namespace() -> String {
    APP_CONFIG.client.clone().and_then(|c| { c.namespace }).unwrap_or(String::from("defaults"))
}



pub fn get_appconfig() -> AppConfig {
    let opt = Opt::from_args();

    let toml_text = fs::read_to_string(opt.defaults_file.clone()).unwrap();
    let config: AppConfig = match toml::from_str(&toml_text) {
        Ok(p) => p,
        Err(e) => {
            panic!("<ParseTOML> File {} cannot be parsed, CAUSE:\n{}", opt.defaults_file, e);
        }
    };
    config
}

fn parse_file_path(p: &str) -> Result<String, io::Error> {
    match File::open(p) {
        Ok(_) => {Ok(String::from(p))},
        Err(e) => {
            Err(Error::new(io::ErrorKind::NotFound, format!("{}: {}", p, e)))
        }
    }
}