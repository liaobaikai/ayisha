use std::{sync::{atomic::AtomicUsize, mpsc, Arc}, time::Duration};

use actix::*;
use actix_web::{ middleware::Logger, web, App, HttpServer
};
use db::Node;

mod server;
mod session;
mod router;
mod servlet;
mod auth;
mod voter;
mod config;
mod db;
mod ws;

// https://github.com/actix/examples/blob/master/websockets/chat/src/main.rs
#[actix_web::main]
async fn main() -> std::io::Result<()> {

    // 查询本地数据库的所有配置是否有效
    log4rs::init_file("log4rs.yml", Default::default()).unwrap();

    log::info!("Server is listen on 0.0.0.0:{}, server_id:{}", config::get_server_port(), config::get_server_id());
    log::info!("Server will be running {} workers", config::get_server_worker());

    let app_state = Arc::new(AtomicUsize::new(0));

    let db = db::DB::new();
    let node = match db.query_node(&format!("{}", config::get_server_id()).as_str()) {
        Some(node) => node,
        None => Node::new()
    };
    let attr = ws::Attribute::new(node);

    // 获取本机信息
    let server = server::ChatServer::new(app_state.clone(), attr).start();

    let http_server = HttpServer::new(move || {
        App::new()
            .configure(router::config)
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .wrap(Logger::new("%{r}a - - \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %t in %Ts").log_target("data"))
    })
    .workers(config::get_server_worker())
    .bind(("0.0.0.0", config::get_server_port()))?
    .run();

    let mut voter_handles = Vec::new();
    // 是否需要启动客户端
    if !config::get_client_singleton() {
        for server in config::get_nodes() {
            // let node_id = format!("{}", node.id);
            let server_addr = server.host.unwrap_or(String::from("127.0.0.1"));
            let server_port = server.port.unwrap_or(config::DEFAULT_PORT);

            let voter_handle = actix::spawn(async move {

                let ws = voter::WebSocket::new(server_addr, server_port, server.id);
                loop {
                    ws.start(&config::get_client_app_key(), &config::get_client_app_secret()).await;
                    log::info!("[{}] - - Connection disconnected, wait for {} seconds", server.id, config::get_client_connect_retry());
                    tokio::time::sleep(Duration::from_secs(config::get_client_connect_retry() as u64)).await;
                }
            });

            voter_handles.push(voter_handle);

        }
    }
    
    http_server.await.unwrap();
    for handle in voter_handles {
        handle.await.unwrap();
    }
    
    println!("OK.");

    Ok(())
}
