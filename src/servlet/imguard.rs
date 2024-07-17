
use std::collections::HashMap;

use actix_web::{guard::{self, Guard}, web::Query};

use crate::auth::{self, imauth::User};

pub const HEADER_APP_KEY: &str = "APP_KEY";
pub const HEADER_APP_SECRET: &str = "APP_SECRET";

pub struct IMGuard;

impl Guard for IMGuard {

    fn check(&self, ctx: &guard::GuardContext<'_>) -> bool {
        let client_ip = ctx.head().peer_addr.unwrap().ip().to_string();

        // 如果参数有值，优先使用参数的值
        // 如果参数没有值，则获取header的值，header无值则无权限
    
        // 通过参数获取
        if let Some(query_str) = ctx.head().uri.query() {

            if let Ok(params) = Query::<HashMap<String, String>>::from_query(query_str) {
                // 包含大小写
                let mut param_app_key: Option<&String> = None;
                let mut param_app_secret: Option<&String> = None;
                for key in params.keys() {
                    if key.eq_ignore_ascii_case(HEADER_APP_KEY) {
                        param_app_key = params.get(key);
                    } else if key.eq_ignore_ascii_case(HEADER_APP_KEY) {
                        param_app_secret = params.get(key);
                    }
                }

                let mut valid = false;
                if let Some(app_key) = param_app_key {
                    // 参数存在，且参数为空
                    valid = valid | !app_key.is_empty();
                }

                if let Some(app_secret) = param_app_secret {
                    valid = valid & !app_secret.is_empty();
                }

                if valid {
                    return auth::imauth::IMAuth::new().authenticate(&User {
                        app_key: param_app_key.unwrap().to_owned(), 
                        app_secret: param_app_secret.unwrap().to_owned(), 
                        ip: client_ip
                    });
                }
            }
        } 
        
        let header_app_key = ctx.head().headers().get(HEADER_APP_KEY);
        let header_app_secret = ctx.head().headers().get(HEADER_APP_SECRET);
        if header_app_key.is_none() || header_app_secret.is_none() {
            // header无参数
            return false;
        }
        
        let mut valid = false;
        // 通过header获取
        if let Some(app_key) = header_app_key {
            valid = valid | !app_key.is_empty();
        }
        if let Some(app_secret) = header_app_secret {
            valid = valid & !app_secret.is_empty();
        }

        if valid {
            return auth::imauth::IMAuth::new().authenticate(&User { 
                app_key: header_app_key.unwrap().to_str().unwrap().to_owned(), 
                app_secret: header_app_secret.unwrap().to_str().unwrap().to_owned(), 
                ip: client_ip 
            })
        }
        
        false
        
    }


}



