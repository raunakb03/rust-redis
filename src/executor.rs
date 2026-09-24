use crate::parser::RedisRes;

#[allow(clippy::collapsible_if)]
pub fn execute(req: RedisRes) -> String {
    if let RedisRes::Array(items) = req {
        if let Some(RedisRes::BulkString(_, command_name)) = items.first() {
            match command_name.to_uppercase().as_str() {
                "PING" => return "+PONG\r\n".to_string(),
                "ECHO" => {
                    if let Some(RedisRes::BulkString(len, arg)) = items.get(1) {
                        return format!("${}\r\n{}\r\n", len, arg);
                    }
                },
                _ => return format!("-ERR unknown command: {}\r\n", command_name),
            }
        }
    }
    "-ERR invalid request\r\n".to_string()
}
