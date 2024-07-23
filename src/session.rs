use std::{net::IpAddr, time::{Duration, Instant}};

use actix::prelude::*;
use actix_web_actors::ws;

use crate::{server::{self}, ws::{WsEvent, WsRequest, WsResponse}};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

// pub enum Action {
//     SessionCreate,

// }

#[derive(Debug)]
pub struct WsChatSession {

    pub server_id: usize,
    // 
    pub voter_id: usize,

    /// unique session id
    pub session_id: usize,

    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    pub hb: Instant,

    /// joined cluster
    pub namespace: String,

    /// peer name
    pub name: Option<String>,

    /// Chat server
    pub addr: Addr<server::ChatServer>,

    /// Is closed connection?
    pub closed: bool,

    pub ip: IpAddr,
}

impl WsChatSession {
    /// helper method that sends ping to client every 5 seconds (HEARTBEAT_INTERVAL).
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        // log::debug!("Start heartbeat timer, NID=`{}`", self.id);
        
        // 如果是debug模式，暂停会导致执行多次
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if act.closed {
                return;
            }

            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                act.closed = true;

                // notify chat server
                act.addr.do_send(server::Disconnect { id: act.session_id, closed: act.closed, ip: act.ip });

                // heartbeat timed out
                log::info!("[{}] - [{}] - The session has been aborted due to heartbeat timed out {:?}", act.server_id, act.voter_id, CLIENT_TIMEOUT);

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });

        log::debug!("[{}] - [{}] - Session Heartbeat Timer started", self.server_id, self.voter_id);
    }
}

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start.
    /// We register ws session with ChatServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        
        self.hb(ctx);

        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsChatSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(server::Connect {
                server_id: self.server_id,
                voter_id: self.voter_id,
                addr: addr.recipient(),
                ip: self.ip,
                namespace: self.namespace.clone(),

            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.session_id = res,
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);

        log::debug!("[{}] - [{}] - Session started", self.server_id, self.voter_id);
            
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(server::Disconnect { id: self.session_id, closed: self.closed, ip: self.ip });
        Running::Stop
    }
}

/// Handle messages from chat server, we simply send it to peer websocket
impl Handler<server::Message> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {

        // log::debug!("==>{:?}", msg);

        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        // log::debug!("==>msg:{:?}", msg);

        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Binary(_) => {
                let rsp = WsResponse::error(WsEvent::UNKNOWN, "Unexpected binary".to_owned(), None);
                ctx.text(rsp.to_string());
            },
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
            ws::Message::Text(text) => {
                let s = &String::from(text);
                if s.is_empty() {
                    let rsp = WsResponse::error(WsEvent::UNKNOWN, "Empty WsRequest".to_owned(), None);
                    ctx.text(rsp.to_string());
                    return;
                }
                
                let v = match WsRequest::from_str(s) {
                    Ok(v) => v,
                    Err(e) => {
                        let rsp = WsResponse::error(WsEvent::UNKNOWN,
                            format!("ParseError: `{}` is not valid JSON on WsRequest, cause: {}", s, e), None);
                        ctx.text(rsp.to_string());
                        return;
                    }
                };

                match v.event {
                    WsEvent::JOIN => {
                        // 加入到集群
                        // 不等待响应
                        // self.addr.do_send(server::Join {
                        //     session_id: self.session_id,
                        //     data: v.data.clone()
                        // });
                        let data = server::Join {
                            session_id: self.session_id,
                            state: v.state.clone()
                        };

                        // 同步发送消息并响应
                        self.addr.send(data).into_actor(self).then(move |res, _, ctx| {
                            if let Ok(join) = res {
                                // 0 => 本机刚恢复,投票周期比加入的节点要小,本机需要同步数据
                                // 1 => 本机投票周期比加入节点的要大,加入节点需要同步数据
                                // 2 => 投票周期一致
                                // let mut data = v.data.clone();
                                // if level == 0 {
                                //     data.status = WsDataStatus::RECV;
                                // } else if level == 1 {
                                //     data.status = WsDataStatus::SEND;
                                // } else {
                                //     data.status = WsDataStatus::YES;
                                // }
                                ctx.text(WsResponse::ok(v.event, "Joined".to_owned(), Some(join.state)).to_string());
                            } else {
                                println!("Something is wrong")
                            }
                            fut::ready(())
                        }).wait(ctx);
                        
                    }

                    WsEvent::COPY => {
                        // 数据同步，一般加入到集群中下一步就是数据同步了
                        // 将数据同步到其他会话

                        // 加入到集群中
                        // 不等待响应
                        self.addr.do_send(server::Copy {
                            session_id: self.session_id,
                            state: v.state.clone()
                        });

                        ctx.text(WsResponse::ok(v.event, "Copy".to_owned(), Some(v.state)).to_string());

                    }

                    WsEvent::VOTE => {
                        // 数据同步，一般加入到集群中下一步就是数据同步了
                        // 将数据同步到其他会话

                        // 加入到集群中
                        // 不等待响应
                        // self.addr.do_send(server::Vote {
                        //     session_id: self.session_id,
                        //     state: v.state.clone()
                        // });

                        // ctx.text(WsResponse::success(v.event, "Election".to_owned(), Some(v.state)).to_string());
                        let data = server::Vote {
                            session_id: self.session_id,
                            state: v.state.clone(),
                            ok: false
                        };
                        self.addr.send(data).into_actor(self).then(move |res, _, ctx| {
                            if let Ok(vote) = res {
                                let state = Some(vote.state.clone());
                                if vote.ok {
                                    // 投票成功
                                    ctx.text(WsResponse::ok(v.event, "Voted".to_owned(), state).to_string());
                                } else {
                                    // 投票失败，票数转移给其他候选人
                                    ctx.text(WsResponse::failed(v.event, "Changed Vote".to_owned(), state).to_string());
                                }
                            } else {
                                println!("Something is wrong")
                            }
                            fut::ready(())
                        }).wait(ctx);

                    }

                    _ => {
                        // 未适配的类型
                        ctx.text(WsResponse::failed(v.event, "Unsupported event".to_owned(), Some(v.state)).to_string());
                        return;
                    }
                }


                // log::debug!("msg: {:?}", v);
                // we check for /sss type of messages
                // if m.starts_with('/') {
                //     let v: Vec<&str> = m.splitn(2, ' ').collect();
                //     match v[0] {
                //         /*
                //         "/list" => {
                //             // Send ListClusters message to chat server and wait for
                //             // response
                //             println!("List clusters");
                //             self.addr
                //                 .send(server::ListClusters)
                //                 .into_actor(self)
                //                 .then(|res, _, ctx| {
                //                     match res {
                //                         Ok(rooms) => {
                //                             for room in rooms {
                //                                 ctx.text(room);
                //                             }
                //                         }
                //                         _ => println!("Something is wrong"),
                //                     }
                //                     fut::ready(())
                //                 })
                //                 .wait(ctx)
                //             // .wait(ctx) pauses all events in context,
                //             // so actor wont receive any new messages until it get list
                //             // of clusters back
                //         } */
                //         // "/join" => {
                //         //     if v.len() == 2 {
                //         //         v[1].clone_into(&mut self.cluster);
                //         //         self.addr.do_send(server::Join {
                //         //             id: self.id,
                //         //             name: self.cluster.clone(),
                //         //         });

                //         //         ctx.text("joined");
                //         //     } else {
                //         //         ctx.text("!!! room name is required");
                //         //     }
                //         // }
                //         "/name" => {
                //             if v.len() == 2 {
                //                 self.name = Some(v[1].to_owned());
                //             } else {
                //                 ctx.text("!!! name is required");
                //             }
                //         }
                //         _ => ctx.text(format!("!!! unknown command: {m:?}")),
                //     }
                // } else {
                //     let msg = if let Some(ref name) = self.name {
                //         format!("{name}: {m}")
                //     } else {
                //         m.to_owned()
                //     };
                //     // send message to chat server
                //     self.addr.do_send(server::ClientMessage {
                //         id: self.id,
                //         msg,
                //         cluster: self.cluster.clone(),
                //     })
                // }
            }
            
        }
    }
}