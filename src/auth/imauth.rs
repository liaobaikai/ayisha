use std::{net::{IpAddr, Ipv4Addr, Ipv6Addr}, str::FromStr};

use ipnetwork::IpNetwork;
use serde::Deserialize;

use crate::config;

// use crate::db;

#[derive(Deserialize, Debug)]
pub struct User {
    pub app_key: String,
    pub app_secret: String,
    pub ip: String,
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

        let ip: IpAddr = match Ipv4Addr::from_str(&user.ip) {
            Ok(ip) => IpAddr::V4(ip),
            Err(_) => {
                match Ipv6Addr::from_str(&user.ip) {
                    Ok(ipv6) => IpAddr::V6(ipv6),
                    Err(_) => {
                        log::error!("- - - invalid client address: {}", &user.ip);
                        return false;
                    }
                 }
            }
        };

        let idents = config::HostBasedAuth::new().match_ident(&user.app_key, &user.app_secret);

        let mut valid_user = false;
        let mut valid_addr = false;

        for (host, _ident) in idents {
            valid_user = true;
            // 判断是否设置了白名单
            // 考虑IPv4和IPv6
            // let host = userauth.host;
            // 有效
            // 未指定，则全部 IP 放通
            valid_addr |= host.is_empty();
            if valid_addr {
                break;
            }

            // 是否匹配
            valid_addr |= match IpNetwork::from_str(&host) {
                Ok(net) => {
                    // 包含，放通
                    // 未指定，则全部 IP 放通
                    // 双方都是回环地址，则放通
                    let mut is_unspecified = ip.is_ipv4() & net.is_ipv4() & net.network().is_unspecified();
                    is_unspecified |= ip.is_ipv6() & net.is_ipv6() & net.network().is_unspecified();

                    let mut is_loopback = ip.is_ipv4() & net.is_ipv4() & net.network().is_loopback();
                    is_loopback |= ip.is_ipv6() & net.is_ipv6() & net.network().is_loopback();

                    log::debug!("[{}] - [{}] - hba_ident: Unspecified: {}, Address Contains IP: {}, Address & IP is Loopback: {}", ip, net, is_unspecified, net.contains(ip), is_loopback, );
                    
                    is_unspecified | net.contains(ip) | is_loopback
                    
                },
                Err(e) => {
                    log::error!("[{}] - - invalid hba_ident.homl address, cause: {}", ip, e);
                    false
                }
            };

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


