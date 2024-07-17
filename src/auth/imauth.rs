use std::{net::{Ipv4Addr, Ipv6Addr}, str::FromStr};

use ipnetwork::{Ipv4Network, Ipv6Network};
use serde::Deserialize;

use crate::db;

#[derive(Deserialize, Debug)]
pub struct User {
    pub app_key: String,
    pub app_secret: String,
    pub ip: String
}

pub struct IMAuth;

impl IMAuth {

    pub fn new() -> Self {
        IMAuth {
            
        }
    }

    // 检查权限
    // 考虑添加跳过鉴权表功能
    pub fn authenticate(&self, user: &User) -> bool {

        let db = db::DB::new();
        let userauths = db.query_userauth(&user.app_key, &user.app_secret);

        let mut valid_user = false;
        let mut valid_addr = false;

        for userauth in userauths {
            valid_user = true;
            // 判断是否设置了白名单
            // 考虑IPv4和IPv6
            let host = userauth.host;
            // 有效
            valid_addr = valid_addr | host.is_empty();

            if valid_addr {
                break;
            }

            // 如果不存在 / 则代表是实际IP
            if host.contains("/") {
                // 如果设置了白名单的话，则放通
                let net = Ipv4Network::from_str(&host).unwrap();

                valid_addr = valid_addr | net.contains(Ipv4Addr::from_str(&user.ip).unwrap());

                if !valid_addr {
                    // IPV6
                    let net = Ipv6Network::from_str(&host).unwrap();
                    valid_addr = valid_addr | net.contains(Ipv6Addr::from_str(&user.ip).unwrap());
                }
                
            }

            // 相等
            if !valid_addr {
                valid_addr = valid_addr | (host.trim() == user.ip.trim())
            }

            // 判断是否到达有效期
            // if userauth.expired_datetime.is_none() {
            //     break;
            // }

            if valid_addr {
                break;
            }

        }

        if !valid_user {
            log::warn!("{} - - Access denied for APP_KEY or APP_SECRET", user.ip);
            return false;
        }

        if !valid_addr {
            log::warn!("{} - - Access denied for ADDRESS", user.ip);
            return false;
        }
    
        true
    }
    
}


