use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

use crate::data_manager::{DataManager, RedisData};
use crate::parser::RespValue;

type CommandResult = Result<RespValue, String>;

pub fn execute(req: RespValue, db: &Mutex<DataManager>) -> RespValue {
    let RespValue::Array(items) = req else {
        return RespValue::Error("ERR invalid request".into());
    };
    let Some(args) = items
        .iter()
        .map(|item| match item {
            RespValue::BulkString(s) => Some(s.as_str()),
            _ => None,
        })
        .collect::<Option<Vec<&str>>>()
    else {
        return RespValue::Error("ERR invalid request".into());
    };
    let Some((name, args)) = args.split_first() else {
        return RespValue::Error("ERR invalid request".into());
    };

    let result = match name.to_ascii_uppercase().as_str() {
        "PING" => ping(args),
        "ECHO" => echo(args),
        "SET" => set(args, db),
        "GET" => get(args, db),
        _ => Err(format!("ERR unknown command '{name}'")),
    };
    result.unwrap_or_else(RespValue::Error)
}

fn wrong_args(command: &str) -> String {
    format!("ERR wrong number of arguments for '{command}' command")
}

fn ping(args: &[&str]) -> CommandResult {
    match args {
        [] => Ok(RespValue::SimpleString("PONG".into())),
        [message] => Ok(RespValue::BulkString(message.to_string())),
        _ => Err(wrong_args("ping")),
    }
}

fn echo(args: &[&str]) -> CommandResult {
    let [message] = args else {
        return Err(wrong_args("echo"));
    };
    Ok(RespValue::BulkString(message.to_string()))
}

fn set(args: &[&str], db: &Mutex<DataManager>) -> CommandResult {
    let [key, value, options @ ..] = args else {
        return Err(wrong_args("set"));
    };

    let mut ttl = None;
    let mut options = options.iter();
    while let Some(option) = options.next() {
        let to_duration = match option.to_ascii_uppercase().as_str() {
            "EX" => Duration::from_secs,
            "PX" => Duration::from_millis,
            _ => return Err("ERR syntax error".into()),
        };
        let amount = options
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .ok_or("ERR invalid expire time in 'set' command")?;
        ttl = Some(to_duration(amount));
    }

    let mut data = RedisData::new(value.to_string());
    if let Some(ttl) = ttl {
        let expiration = Instant::now()
            .checked_add(ttl)
            .ok_or("ERR invalid expire time in 'set' command")?;
        data = data.with_expiration(expiration);
    }
    db.lock().unwrap().insert(key.to_string(), data);
    Ok(RespValue::SimpleString("OK".into()))
}

fn get(args: &[&str], db: &Mutex<DataManager>) -> CommandResult {
    let [key] = args else {
        return Err(wrong_args("get"));
    };
    Ok(match db.lock().unwrap().get(key) {
        Some(value) => RespValue::BulkString(value.clone()),
        None => RespValue::Null,
    })
}
