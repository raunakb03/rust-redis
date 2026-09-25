use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

use crate::parser::RedisRes;
use crate::data_manager::{self, RedisData};

#[allow(clippy::collapsible_if)]
pub fn execute(req: RedisRes, db: &Arc<Mutex<data_manager::DataManager>>) -> String {
    if let RedisRes::Array(items) = req {
        if let Some(RedisRes::BulkString(_, command_name)) = items.first() {
            match command_name.to_uppercase().as_str() {
                "PING" => return "+PONG\r\n".to_string(),
                "ECHO" => {
                    if let Some(RedisRes::BulkString(len, arg)) = items.get(1) {
                        return format!("${}\r\n{}\r\n", len, arg);
                    }
                },
                "SET" => {
                    if let Some(RedisRes::BulkString(_, key)) = items.get(1) && let Some(RedisRes::BulkString(_, value)) = items.get(2) {
                        let mut expiry_ms: Option<u64> = None;
                        let mut i = 3;
                        while i < items.len() {
                            if let RedisRes::BulkString(_, set_option) = &items[i] {
                                match set_option.to_uppercase().as_str() {
                                    "EX" => {
                                        if let Some(RedisRes::BulkString(_, secs)) = &items.get(i+1) {
                                            if let Ok(secs) = secs.parse::<u64>() {
                                                expiry_ms = Some(secs*1000);
                                            }
                                        }
                                        i += 2;
                                    },
                                    "PX" => {
                                        if let Some(RedisRes::BulkString(_, secs)) = &items.get(i+1) {
                                            if let Ok(secs) = secs.parse::<u64>() {
                                                expiry_ms = Some(secs);
                                            }
                                        }
                                        i += 2;
                                    },
                                    _ => {
                                        i += 1;
                                    }
                                }
                            } else {
                                i += 1;
                            }
                        }
                        let mut manager = db.lock().unwrap();
                        let mut val = RedisData::new(value.to_string());
                        if expiry_ms.is_some() {
                            val = val.with_expiration(Instant::now() + Duration::from_millis(expiry_ms.unwrap_or(0)));
                        }
                        manager.insert(String::from(key), val);
                        return "+OK\r\n".to_string();
                    }
                },
                "GET" => {
                    if let Some(RedisRes::BulkString(_, key)) = items.get(1) {
                        let mut manager = db.lock().unwrap();
                        if let Some(res) = manager.get(key) {
                            return format!("${}\r\n{}\r\n", res.len(), res);
                        } else {
                            return "$-1\r\n".to_string();
                        }
                    }
                }
                _ => return format!("-ERR unknown command: {}\r\n", command_name),
            }
        }
    }
    "-ERR invalid request\r\n".to_string()
}
