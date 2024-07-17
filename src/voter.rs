use std::{collections::HashMap, sync::Arc, time::Duration};

use awc::ws::{Frame, Message};
use bytestring::ByteString;
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::{select, sync::mpsc::{self}, time::{self, Instant}};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::{config, db::{self, Node}, servlet, ws::{self, Attribute, WsDataStatus, WsEvent, WsRequest, WsResponse, WsStatusCode}};


// fn handle_message(id: &str, msg: Bytes, request: &mut Arc<WsRequest>) -> Option<ByteString> {
//     let raw = String::from_utf8_lossy(msg.as_ref()).to_string();
//     log::debug!("[{}] - - handle_message: WsResponse: {}", id, raw);
//     let v = match WsResponse::from_str(&raw) {
//         Ok(v) => v,
//         Err(e) => {
//             log::error!("[{}] - - handle_message: ParseError: `{}` is not valid JSON, CAUSE: {}", id, raw, e);
//             return None;
//         }
//     };

//     match v.status_code {
//         WsStatusCode::SUCCESS => {
//         }
//         WsStatusCode::FAILED | WsStatusCode::ERROR => {
//             log::error!("[{}] - - handle_message: {}", id, v.mail_box);
//             return None;
//         }
//     }

//     // let mut request = request.as_ref();

//     // 请求成功
//     match v.event {
//         ws::WsEvent::JOIN => {
//             // 进入数据同步
//             if let Some(data) = v.data {
//                 match data.status {
//                     WsDataStatus::RECV | WsDataStatus::SEND => {
//                         // 同步数据
//                         // let mut request = ws::WsRequest {
//                         //     event: ws::WsEvent::COPY,
//                         //     data
//                         // };
//                         // request.event = ws::WsEvent::COPY;

                
//                         // return Some(build_message(&mut request));
//                         return None;
//                     }
//                     WsDataStatus::NONTODO => {
//                         // 
//                     }
//                 }
//             }
//         }
//         ws::WsEvent::COPY => {
//         }
//         ws::WsEvent::VOTE => {
//         }
//         ws::WsEvent::UNKNOWN => {
//         }
//     }

//     None

// }


// 发送消息到其他节点
// 1. 只发送本机的信息
// fn build_message(request: &mut Arc<WsRequest>) -> ByteString {
//     let request = request.as_ref();

//     log::debug!("[{}] - - build_message: WsRequest: {}", request.data.id, serde_json::to_string(request).unwrap());
//     let buf: ByteString = serde_json::to_string(request).unwrap().to_owned().into();
//     match request.event {
//         ws::WsEvent::JOIN => {
//             // 进入数据同步
//             // data.event = WsEvent::COPY;
//         }
//         ws::WsEvent::COPY => {
//             // data.event = WsEvent::VOTE;
//         }
//         ws::WsEvent::VOTE => {
//         }
//         ws::WsEvent::UNKNOWN => {
//         }
//     }

//     buf
// }

pub struct WebSocket {
    addr: String,
    port: u16,
    server_id: usize,
    attr: Attribute
}

impl WebSocket {

    pub fn new(addr: String, port: u16, server_id: usize) -> Self {

        // 将本节点的信息同步到其他节点
        let db = db::DB::new();
        let node = match db.query_node(&format!("{}", server_id)) {
            Some(n) => n,
            None => Node::new()
        };

        let mut attr = ws::Attribute::new(node);
        // 有效票数:1
        // 每次运行,需持有1票
        attr.poll = 1;
        attr.term = 10;

        WebSocket {
            addr,
            port,
            server_id,
            attr
        }
    }

    pub async fn start(&self, app_key: &str, app_secret: &str) {
        
        // 正向:从定时器发送数据,并到ws接收数据
        let (tx, _rx) = mpsc::unbounded_channel::<ByteString>();
        // 反向:从ws接收,并传输到定时器
        let (reverse_tx, _reverse_rx) = mpsc::unbounded_channel::<WsResponse>();
        let mut rx = UnboundedReceiverStream::new(_rx);
        let mut reverse_rx = UnboundedReceiverStream::new(_reverse_rx);
        // let tx1 = tx.clone();

        let token = CancellationToken::new();
        let cloned_token = token.clone();
        // let inner_id = String::from(id);
        let server_id = Arc::new(format!("{}", self.server_id));
        let cloned_server_id = Arc::clone(&server_id);
        // let inner_id2 = String::from(id);
        
        let mut c_attr = self.attr.clone();
        
        // 启动定时发送任务
        let send_handle = tokio::spawn(async move {
            let start = Instant::now();
            // let start = Instant::now() + Duration::from_secs(5);
            let interval = Duration::from_secs(10);
            let mut intv = time::interval_at(start, interval);
            // 投票箱
            // server_id, poll
            let polling_box: HashMap<usize, usize> = HashMap::new();
            
            loop {
                select! {
                    _ = cloned_token.cancelled() => {
                        break;
                    }
                    Some(v) = reverse_rx.next() => {
                        // v => WsResponse
                        log::debug!("ws_response: {:?}", v);
                        // 数据分析
                        match v.event {
                            ws::WsEvent::JOIN => {
                                // 进入数据同步
                                if let Some(data) = v.data {
                                    c_attr.poll = data.poll;

                                    match data.status {
                                        WsDataStatus::NONTODO => {
                                            continue
                                        }

                                        WsDataStatus::RECV | WsDataStatus::SEND => {
                                            // 进入数据同步环节
                                            // c_attr.event = ws::WsEvent::COPY;
                                            // let src = c_attr.to_bytestr();
                                            // if let Err(e) = tx.send(src) {
                                            //     log::error!("[{}] - - TX send failed, cause: {}", &inner_id, e);
                                            //     break;
                                            // }
                                        }
                                    }
                                }
                            }
                            ws::WsEvent::COPY => {
                                // 其他节点同步数据到本节点
                                if let Some(data) = v.data {
                                    match data.status {
                                        WsDataStatus::NONTODO => {
                                            continue
                                        }

                                        WsDataStatus::RECV => {
                                            // 数据需要从节点节点同步到本节点
                                            // local_request.event = WsEvent::COPY;
                                            log::debug!("[{}] ### RECV data:: {:?}", &server_id, data);

                                        }

                                        WsDataStatus::SEND => {
                                            // 数据同步到其他节点
                                        }
                                    }
                                }
                            }
                            ws::WsEvent::ELECTION => {
                                // 选举开始
                            }

                            ws::WsEvent::BROADCAST => {
                                // 接收广播数据（投票传递）
                                if let Some(data) = v.data {
                                    // 判断投票情况
                                    // 1、如果对方的数据比我要新，则需要改票
                                    if data.tranx > c_attr.tranx {
                                        // 对方的数据比我新
                                    }

                                }
                            }

                            ws::WsEvent::UNKNOWN => {
                            }
                        }
                        
                    }
                    _ = intv.tick() => {
                        
                        let src = WsRequest{
                            event: WsEvent::JOIN,
                            attr: c_attr.clone()
                        }.to_bytestr();
                        if let Err(e) = tx.send(src) {
                            log::error!("[{}] - - TX send failed, cause: {}", &server_id, e);
                            break;
                        }
                        log::debug!("[{}] - - TX send, sleep for 10 seconds to continue", &server_id);
                    }
                }
            }
        });


        let url = format!("ws://{}:{}/im/chat/{}/{}", self.addr, self.port, config::get_client_namespace(), self.server_id);
        log::info!("[{}] - - Connecting to {}:{}", self.server_id, self.addr, self.port);

        let (_resp, mut ws) = match awc::Client::new().ws(&url).header(servlet::imguard::HEADER_APP_KEY, app_key).header(servlet::imguard::HEADER_APP_SECRET, app_secret).connect().await {
            Ok((a, b)) => (a, b),
            Err(e) => {
                log::error!("[{}] - - Failed to connect to server, cause: {}", self.server_id, e);
                token.cancel();
                return
            }
        };

        log::info!("[{}] - - Connected to {}:{}", self.server_id, self.addr, self.port);
        
        loop {
            tokio::select! {
                // 接收信息
                Some(payload) = ws.next() => {
                    let payload = match payload {
                        Ok(f) => f,
                        Err(_) => return
                    };
                    match payload {
                        Frame::Text(msg) => {

                            // 接收数据解析
                            let raw = String::from_utf8_lossy(msg.as_ref()).to_string();
                            log::debug!("[{}] - - handle_message: WsResponse: {}", self.server_id, raw);
                            let v = match WsResponse::from_str(&raw) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!("[{}] - - handle_message: ParseError: `{}` is not valid JSON, CAUSE: {}", self.server_id, raw, e);
                                    return
                                }
                            };
                            // 状态码判断
                            match v.status_code {
                                WsStatusCode::SUCCESS => {
                                }
                                WsStatusCode::FAILED | WsStatusCode::ERROR => {
                                    log::error!("[{}] - - handle_message: {}", self.server_id, v.mail_box);
                                    return
                                }
                            }
                            
                            // 将数据传回定时器
                            if let Err(e) = reverse_tx.send(v) {
                                log::error!("[{}] - - reverse_TX send failed, cause: {}", &cloned_server_id, e);
                            }
                        }
                        Frame::Ping(msg) => {
                            // log::debug!("[{}] - - Ping: {:?}", id, msg);
                            ws.send(Message::Pong(msg)).await.unwrap();
                        }
                        Frame::Pong(msg) => {
                            log::debug!("[{}] - - Pong: {:?}", self.server_id, msg);
                        }
                        Frame::Continuation(msg) => {
                            log::debug!("[{}] - - Continuation: {:?}", self.server_id, msg);
                        }
                        Frame::Close(msg) => {
                            log::debug!("[{}] - - Connection closed, cause: {}", self.server_id, msg.unwrap().description.unwrap());
                            break;
                        }
                        Frame::Binary(msg) => {
                            log::debug!("[{}] - - Binary: {:?}", self.server_id, msg);
                        }
                    }

                },

                // 定时发送信息
                Some(buf) = rx.next() => {
                    if let Err(e) = ws.send(Message::Text(buf)).await {
                        log::info!("[{}] - - WsRequest send failed, cause: {}", self.server_id, e);
                        break;
                    }
                },

            }

        }

        // drop
        token.cancel();

        // 将send_handle添加到任务中
        let _ = tokio::join!(send_handle);

    }
}

