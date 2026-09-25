use std::sync::{Arc, Mutex};

use crate::parser::RedisRes;
use crate::dataManager;

#[allow(clippy::collapsible_if)]
pub fn execute(req: RedisRes, db: &mut Arc<Mutex<dataManager::DataManager>>) -> String {
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
                        let mut manager = db.lock().unwrap();
                        manager.insert(String::from(key), String::from(value));
                        return "+OK\r\n".to_string();
                    }
                },
                "GET" => {
                    if let Some(RedisRes::BulkString(_, key)) = items.get(1) {
                        let manager = db.lock().unwrap();
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
