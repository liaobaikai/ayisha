use std::{process::exit, sync::{atomic::{AtomicUsize, Ordering}, Arc}, thread, time::Duration};

use actix::*;
use actix_web::{ middleware::Logger, web, App, HttpServer
};
use parking_lot::{Condvar, Mutex};
use pom::PoManager;
use shared::ShareGlobalArea;
// use db::Node;

mod server;
mod session;
mod router;
mod servlet;
mod auth;
mod vot;
mod config;
mod ws;
mod pom;
mod shared;



// https://github.com/actix/examples/blob/master/websockets/chat/src/main.rs
#[actix_web::main]
async fn main() -> std::io::Result<()> {

    // let pair = Arc::new((Mutex::new(false), Condvar::new()));
    // let data1 = server::GLOBAL_STATE_DATA.clone();
    /*
    let mut sga = ShareGlobalArea::new();
    let mut flag = false;

    let pair2 = shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
    thread::spawn(move|| {
        let &(ref lock, ref cvar) = &*pair2;
        println!("inner waiting lock");
        let mut data = lock.lock();
        println!("inner acquire lock");
        println!("inner data.poll=>{}", data.poll);
        sga.poll = 1;
        // 修改数据
        *data = sga;
        let mut started = data.lock.lock();
        *started = true;

        cvar.notify_one();
        println!("cvar.notify_one");
    });
    // 
    {
        println!("wait for the thread to start up");
        let &(ref lock, ref cvar) = &*shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
        println!("outer waiting lock");
        let mut data = lock.lock();
        println!("outer acquire lock");
        // 修改数据
        let mut sga2 = sga.clone();
        sga2.poll += 200;
        *data = sga2;

        let started = data.lock.lock();

        // 不符合条件，则等待
        while !*started {
            println!("cvar wait");
            cvar.wait(&mut data);
            println!("data.poll=>{}", data.poll);
        }
        
    }

    */

    // thread::sleep(Duration::from_secs(10));

    // let pair3 = shared::SHARE_GLOBAL_AREA_MUTEX_PAIR.clone();
    // thread::spawn(move|| {
    //     let &(ref lock, ref cvar) = &*pair3;
    //     println!("inner2 waiting lock");
    //     let mut state = lock.lock();
    //     println!("inner2 acquire lock");
    //     state.poll += 2;
    //     state.lock = true;
    //     cvar.notify_one();
    //     println!("inner2 cvar.notify_one...");
    // });


    // exit(0);

    // 查询本地数据库的所有配置是否有效
    log4rs::init_file("log4rs.yml", Default::default()).unwrap();

    log::info!("[{}] - - Server is listen on 0.0.0.0:{}", config::get_server_id(), config::get_server_port());
    log::info!("[{}] - - Server will be running {} workers", config::get_server_id(), config::get_server_worker());

    let app_state = Arc::new(AtomicUsize::new(0));

    // 服务端的票数
    // let vote = Arc::new(AtomicUsize::new(0));

    let pom = PoManager::new();

    // 获取本机信息
    let server = server::ChatServer::new(app_state.clone(), pom.clone()).start();

    let http_server = HttpServer::new(move || {
        App::new()
            .configure(router::config)
            // .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .wrap(Logger::new("%{r}a - - \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %t in %Ts").log_target("data"))
    })
    .workers(config::get_server_worker())
    .bind(("0.0.0.0", config::get_server_port()))?
    .run();

    let mut voter_handles = Vec::new();
    // 是否需要启动客户端
    if !config::get_client_singleton() {

        // 读取本机有多少投票
        // 预设为 1 票
        // let votes = app_state.clone();
        // votes.fetch_add(1, Ordering::SeqCst);

        // let cloned_pom = pom.clone();
        // let local_server_id = cloned_pom.v_state.id as u16;
        // let voter_handle = actix::spawn(async move {
        //     let vh = vot::VoteHandler::new(cloned_pom, "127.0.0.1".to_owned(), local_server_id, local_server_id as usize);
        //     loop {
        //         let connect_failed = vh.start(&config::get_client_app_key(), &config::get_client_app_secret()).await;
        //         if connect_failed {
        //             log::info!("[{}] - [{}] - Connection not established, wait for {} seconds to retry again", local_server_id as usize, local_server_id as usize, config::get_client_connect_retry());
        //         } else {
        //             log::info!("[{}] - [{}] - The connection has been disconnected, wait for {} seconds to reconnect", local_server_id as usize, local_server_id as usize, config::get_client_connect_retry());
        //         }
        //         tokio::time::sleep(Duration::from_secs(config::get_client_connect_retry() as u64)).await;
        //     }
        // });
        // voter_handles.push(voter_handle);

        // 同时只有一个在

        // let semaphore = Arc::new(Semaphore::new(3));
        

        for server in config::get_nodes() {
            // let node_id = format!("{}", node.id);
            let cloned_pom = pom.clone();
            let server_id = server.id;
            let local_id = cloned_pom.v_state.id;
            // if server_id as usize == local_id {
            //     continue;
            // }
            let server_addr = server.host.unwrap_or(String::from("127.0.0.1"));
            let server_port = server.port.unwrap_or(config::DEFAULT_PORT);
            
            let voter_handle = actix::spawn(async move {

                let vh = vot::VoteHandler::new(cloned_pom, server_addr, server_port, server_id);
                loop {
                    let connect_failed = vh.start(&config::get_client_app_key(), &config::get_client_app_secret()).await;
                    if connect_failed {
                        log::info!("[{}] - [{}] - Connection not established, wait for {} seconds to retry again", local_id, server.id, config::get_client_connect_retry());
                    } else {
                        log::info!("[{}] - [{}] - The connection has been disconnected, wait for {} seconds to reconnect", local_id, server.id, config::get_client_connect_retry());
                    }
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
