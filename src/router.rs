use crate::{server, servlet, session};
use actix::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::time::Instant;

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
    params: web::Path<(usize, usize)>,
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::ChatServer>>,
) -> Result<HttpResponse, Error> {
    let ip = req.peer_addr().unwrap().ip();
    // let namespace = params.0.clone();

    // let mut pass = false;
    // for ns in config::get_server_namespaces().iter() {
    //     if ns.eq_ignore_ascii_case(&namespace) {
    //         pass = true;
    //         break;
    //     }
    // }

    // if !pass {
    //     // 无效的连接
    //     return Err(actix_web::error::ErrorForbidden(format!("Invalid path parameter: namespace=`{}`", namespace)));
    // }

    let server_id = params.0;
    let voter_id = params.1;

    log::debug!(
        "[{}] - {} - Connection ready, endpoint: {}",
        server_id,
        ip,
        req.full_url()
    );
    ws::start(
        session::WsChatSession {
            server_id,
            voter_id,
            session_id: 0,
            hb: Instant::now(),
            // namespace,
            // name: None,
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
    cfg.service(
        web::resource("/im/chat/{server_id}/{voter_id}")
            .guard(servlet::imguard::IMGuard)
            .to(chat_route),
    )
    .default_service(
        web::route()
            .guard(servlet::imguard::IMGuard)
            .to(HttpResponse::Unauthorized),
    );
    // .route("/count", web::get().to(get_count));
}
