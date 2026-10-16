use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use tokio::time::Instant;

use crate::data_manager::{DataError, DataManager, RedisData, RedisDataType};
use crate::parser::RespValue;

type CommandResult = Result<RespValue, String>;

pub fn execute(req: RespValue, db: &Mutex<DataManager>) -> RespValue {
    let RespValue::Array(items) = req else {
        return RespValue::Error("ERR invalid request".into());
    };
    let Some(args) = items
        .iter()
        .map(|item| match item {
            RespValue::BulkString(s) => Some(s),
            _ => None,
        })
        .collect::<Option<Vec<&Bytes>>>()
    else {
        return RespValue::Error("ERR invalid request".into());
    };
    let Some((name, args)) = args.split_first() else {
        return RespValue::Error("ERR invalid request".into());
    };

    let result = match name.to_ascii_uppercase().as_slice() {
        b"PING" => ping(args),
        b"ECHO" => echo(args),
        b"SET" => set(args, db),
        b"GET" => get(args, db),
        b"RPUSH" => rpush(args, db),
        _ => Err(format!(
            "ERR unknown command '{}'",
            String::from_utf8_lossy(name)
        )),
    };
    result.unwrap_or_else(RespValue::Error)
}

fn wrong_args(command: &str) -> String {
    format!("ERR wrong number of arguments for '{command}' command")
}

fn data_error(err: DataError) -> String {
    match err {
        DataError::WrongType => {
            "WRONGTYPE Operation against a key holding the wrong kind of value".into()
        }
    }
}

fn ping(args: &[&Bytes]) -> CommandResult {
    match args {
        [] => Ok(RespValue::SimpleString("PONG".into())),
        [message] => Ok(RespValue::BulkString(Bytes::clone(message))),
        _ => Err(wrong_args("ping")),
    }
}

fn echo(args: &[&Bytes]) -> CommandResult {
    let [message] = args else {
        return Err(wrong_args("echo"));
    };
    Ok(RespValue::BulkString(Bytes::clone(message)))
}

fn set(args: &[&Bytes], db: &Mutex<DataManager>) -> CommandResult {
    let [key, value, options @ ..] = args else {
        return Err(wrong_args("set"));
    };

    let mut ttl = None;
    let mut options = options.iter();
    while let Some(option) = options.next() {
        let to_duration = match option.to_ascii_uppercase().as_slice() {
            b"EX" => Duration::from_secs,
            b"PX" => Duration::from_millis,
            _ => return Err("ERR syntax error".into()),
        };
        let amount = options
            .next()
            .and_then(|n| std::str::from_utf8(n).ok())
            .and_then(|n| n.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .ok_or("ERR invalid expire time in 'set' command")?;
        ttl = Some(to_duration(amount));
    }

    let mut data = RedisData::new(Bytes::clone(value));
    if let Some(ttl) = ttl {
        let expiration = Instant::now()
            .checked_add(ttl)
            .ok_or("ERR invalid expire time in 'set' command")?;
        data = data.with_expiration(expiration);
    }
    db.lock().unwrap().insert(Bytes::clone(key), data);
    Ok(RespValue::SimpleString("OK".into()))
}

fn get(args: &[&Bytes], db: &Mutex<DataManager>) -> CommandResult {
    let [key] = args else {
        return Err(wrong_args("get"));
    };
    Ok(match db.lock().unwrap().get(key) {
        Some(RedisDataType::Byte(bytes)) => RespValue::BulkString(bytes.clone()),
        Some(RedisDataType::List(_)) => todo!(),
        None => RespValue::Null,
    })
}

fn rpush(args: &[&Bytes], db: &Mutex<DataManager>) -> CommandResult {
    let [key, args @ ..] = args else {
        return Err(wrong_args("rpush"));
    };
    if args.is_empty() {
        return Err(wrong_args("rpush"));
    }

    let mut db = db.lock().unwrap();
    let mut len = 0;
    for arg in args {
        len = db
            .insert_into_list(Bytes::clone(key), Bytes::clone(arg))
            .map_err(data_error)?;
    }
    Ok(RespValue::Integer(len as i64))
}
