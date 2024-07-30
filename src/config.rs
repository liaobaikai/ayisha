use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::{env, fs};
use std::{fs::File, io};
use structopt::StructOpt;

lazy_static! {
    pub static ref APP_CONFIG: Config = get_config();
}

pub static DEFAULT_PORT: u16 = 6969;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // 服务端
    pub server: Option<ConfigServer>,
    // 客户端
    pub client: Option<ConfigClient>,
    // 心跳信息
    pub heartbeat: Option<ConfigHeartbeat>,
    // 集群服务节点
    pub node: Option<Vec<ConfigNode>>,
    //
    pub watcher: Option<ConfigWatcher>,
    // pub guard: Option<Vec<AppGuard>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigServer {
    // ID：默认为1
    pub server_id: Option<usize>,
    // 启动端口：默认为6969
    pub port: Option<u16>,
    // 启动线程数：默认为2
    pub worker: Option<usize>,
    // 权重：默认为1
    pub weight: Option<usize>,
    // 数据文件目录
    pub data_root: Option<String>,
    // 如果leader选举一直等不到过半的节点存活，则超过这个时间，就退出，避免脑裂
    pub discovery_wait_timeout: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigClient {
    // 是否跟随server启动，默认为false
    pub singleton: Option<bool>,
    // APP_KEY
    pub app_key: Option<String>,
    // APP_SECRET
    pub app_secret: Option<String>,
    // 连接丢失的情况下 重新尝试连接主服务器之前睡眠的秒数
    pub connect_retry: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigWatcher {
    // 基础目录
    pub root: Vec<ConfigWatchItem>,
    // 目录
    pub dirs: Vec<String>,
    // 变化的文件、路径
    pub vars: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigWatchItem {
    // 源
    pub src: Option<String>,
    // 目标
    pub dst: Option<String>,
    // 扩展
    pub ext: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigHeartbeat {
    // 心跳间隔，秒
    pub interval: Option<usize>,
    // 心跳超时，秒
    pub timeout: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigNode {
    // ID，默认为1
    pub id: usize,
    // 服务节点，默认127.0.0.1
    pub host: Option<String>,
    // 服务端口，默认6969
    pub port: Option<u16>,
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
}

impl HostBasedAuth {
    pub fn new() -> Self {
        let opt = Opt::from_args();

        let toml_text = fs::read_to_string(&opt.hba_ident_file).unwrap();
        let hba: HostBasedAuth = match toml::from_str(&toml_text) {
            Ok(p) => p,
            Err(e) => {
                panic!("<ParseTOML> File {} cannot be parsed, cause:\n{}", &opt.hba_ident_file, e);
            }
        };
        hba
    }

    pub fn match_ident(&self, app_key: &str, app_secret: &str) -> HashMap<String, Ident> {
        let mut ret = HashMap::new();

        for (host, idents) in self.map.iter() {
            for ident in idents {
                if ident.app_key.eq_ignore_ascii_case(app_key)
                    && ident.app_secret.eq_ignore_ascii_case(app_secret)
                {
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

    #[structopt(long, parse(try_from_str=parse_file_path))]
    pub hba_ident_file: String,
}

// SERVER
pub fn get_server_id() -> usize {
    APP_CONFIG
        .server
        .clone()
        .and_then(|s: ConfigServer| s.server_id)
        .unwrap_or(1)
}

pub fn get_server_port() -> u16 {
    APP_CONFIG
        .server
        .clone()
        .and_then(|s: ConfigServer| s.port)
        .unwrap_or(DEFAULT_PORT)
}

pub fn get_server_worker() -> usize {
    APP_CONFIG
        .server
        .clone()
        .and_then(|s: ConfigServer| s.worker)
        .unwrap_or(2)
}

pub fn get_server_discovery_wait_timeout() -> usize {
    APP_CONFIG
        .server
        .clone()
        .and_then(|s: ConfigServer| s.discovery_wait_timeout)
        .unwrap_or(60)
}

pub fn get_server_weight() -> usize {
    APP_CONFIG
        .server
        .clone()
        .and_then(|s: ConfigServer| s.weight)
        .unwrap_or(1)
}

pub fn get_server_data_root() -> PathBuf {
    let pwd = env::current_dir().unwrap();
    let default_data_root = pwd.join(format!(".{}", env!("CARGO_PKG_NAME")));
    let data_root = APP_CONFIG
        .server
        .clone()
        .and_then(|s: ConfigServer| s.data_root)
        .unwrap_or(default_data_root.display().to_string());

    let path = Path::new(&data_root).to_path_buf();
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn get_server_root() -> PathBuf {
    let path = get_server_data_root().join(format!("{}", get_server_id()));
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn get_nodes() -> Vec<ConfigNode> {
    let nodes = APP_CONFIG.node.clone().unwrap_or(Vec::new());
    if nodes.len() < 3 {
        panic!("[[node]] At least 3 nodes");
    }
    nodes
}

// CLIENT
pub fn get_client_singleton() -> bool {
    APP_CONFIG
        .client
        .clone()
        .and_then(|c| c.singleton)
        .unwrap_or(false)
}

pub fn get_client_app_key() -> String {
    APP_CONFIG
        .client
        .clone()
        .and_then(|c| c.app_key)
        .unwrap_or(String::new())
}

pub fn get_client_app_secret() -> String {
    APP_CONFIG
        .client
        .clone()
        .and_then(|c| c.app_secret)
        .unwrap_or(String::new())
}

pub fn get_client_connect_retry() -> usize {
    APP_CONFIG
        .client
        .clone()
        .and_then(|c| c.connect_retry)
        .unwrap_or(60)
}

pub fn get_config() -> Config {
    let opt = Opt::from_args();

    let toml_text = fs::read_to_string(opt.defaults_file.clone()).unwrap();
    let config: Config = match toml::from_str(&toml_text) {
        Ok(p) => p,
        Err(e) => {
            panic!(
                "<ParseTOML> File {} cannot be parsed, cause:\n{}",
                opt.defaults_file, e
            );
        }
    };
    config
}

fn parse_file_path(p: &str) -> Result<String, io::Error> {
    match File::open(p) {
        Ok(_) => Ok(String::from(p)),
        Err(e) => Err(Error::new(io::ErrorKind::NotFound, format!("{}: {}", p, e))),
    }
}
