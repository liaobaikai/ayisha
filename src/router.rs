use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use actix_web::{ web, Error, HttpRequest, HttpResponse, Responder};
use actix::*;
use actix_web_actors::ws;

use crate::{config, server, servlet, session};



// async fn index() -> impl Responder {
//     NamedFile::open_async("./static/index.html").await.unwrap()
// }

/// Displays state
// async fn get_count(count: web::Data<AtomicUsize>) -> impl Responder {
//     let current_count = count.load(Ordering::SeqCst);
//     format!("Visitors: {current_count}")
// }


// ws://<path>/im/chat/123456?app_id=yyy&app_key=xxx
async fn chat_route(
    params: web::Path<(String, usize)>,
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::ChatServer>>,
) -> Result<HttpResponse, Error> {

    let ip = req.peer_addr().unwrap().ip();

    // let nid;
    // match args.parse::<usize>() {
    //     Ok(i) => {
    //         nid = i;

    //     },
    //     Err(_) => {
    //         log::error!("Request from {}, Connection not ready, Received invalid parameters: id=`{}`", ipaddr, args.1);
    //         return Ok(HttpResponse::Forbidden().finish());
    //     }
    // };

    let namespace = params.0.clone();
    // match namespace.as_str() {
    //     "main" | "defaults" => {},
    //     _ => {

    //     }
    // }
    if namespace != config::get_server_namespace() {
        // 
    }


    let id = params.1;

    ws::start(
        session::WsChatSession {
            id,
            session_id: 0,
            hb: Instant::now(),
            namespace,
            name: None,
            addr: srv.get_ref().clone(),
            closed: false,
            ip,
        },
        &req,
        stream,
    )
}

pub fn config(cfg: &mut web::ServiceConfig) {

    // cfg.service(web::resource("/test")
    //     .route(web::get().to(|| HttpResponse::Ok()))
    //     .route(web::head().to(|| HttpResponse::MethodNotAllowed()))
    // )
    // .service(web::resource("/").to(index))
    // .service(Files::new("/static", "./static"))
    cfg.service(web::resource("/im/chat/{namespace}/{id}").guard(servlet::imguard::IMGuard).to(chat_route))
    .default_service(
        web::route().guard(servlet::imguard::IMGuard).to(HttpResponse::Unauthorized)
    );
    // .route("/count", web::get().to(get_count));
}