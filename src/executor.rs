use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
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
        Some(value) => RespValue::BulkString(value.clone()),
        None => RespValue::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(db: &Mutex<DataManager>, args: &[&str]) -> RespValue {
        let request = RespValue::Array(
            args.iter()
                .map(|arg| RespValue::BulkString(Bytes::copy_from_slice(arg.as_bytes())))
                .collect(),
        );
        execute(request, db)
    }

    fn new_db() -> Mutex<DataManager> {
        Mutex::new(DataManager::new())
    }

    fn ok() -> RespValue {
        RespValue::SimpleString("OK".into())
    }

    fn bulk(s: &str) -> RespValue {
        RespValue::BulkString(Bytes::copy_from_slice(s.as_bytes()))
    }

    fn error(msg: &str) -> RespValue {
        RespValue::Error(msg.into())
    }

    // --- request shape ---

    #[test]
    fn rejects_non_array_request() {
        assert_eq!(
            execute(bulk("PING"), &new_db()),
            error("ERR invalid request")
        );
    }

    #[test]
    fn rejects_empty_array() {
        assert_eq!(
            execute(RespValue::Array(vec![]), &new_db()),
            error("ERR invalid request")
        );
    }

    #[test]
    fn rejects_non_bulk_string_arguments() {
        let request = RespValue::Array(vec![bulk("ECHO"), RespValue::Integer(1)]);
        assert_eq!(execute(request, &new_db()), error("ERR invalid request"));
    }

    #[test]
    fn unknown_command() {
        assert_eq!(run(&new_db(), &["FOO"]), error("ERR unknown command 'FOO'"));
    }

    #[test]
    fn command_names_are_case_insensitive() {
        assert_eq!(
            run(&new_db(), &["ping"]),
            RespValue::SimpleString("PONG".into())
        );
        assert_eq!(run(&new_db(), &["eChO", "hi"]), bulk("hi"));
    }

    // --- PING / ECHO ---

    #[test]
    fn ping() {
        let db = new_db();
        assert_eq!(run(&db, &["PING"]), RespValue::SimpleString("PONG".into()));
        assert_eq!(run(&db, &["PING", "hello"]), bulk("hello"));
        assert_eq!(
            run(&db, &["PING", "a", "b"]),
            error("ERR wrong number of arguments for 'ping' command")
        );
    }

    #[test]
    fn echo() {
        let db = new_db();
        assert_eq!(run(&db, &["ECHO", "hey"]), bulk("hey"));
        assert_eq!(
            run(&db, &["ECHO"]),
            error("ERR wrong number of arguments for 'echo' command")
        );
        assert_eq!(
            run(&db, &["ECHO", "a", "b"]),
            error("ERR wrong number of arguments for 'echo' command")
        );
    }

    // --- SET / GET ---

    #[test]
    fn set_then_get() {
        let db = new_db();
        assert_eq!(run(&db, &["SET", "k", "v"]), ok());
        assert_eq!(run(&db, &["GET", "k"]), bulk("v"));
    }

    #[test]
    fn set_overwrites() {
        let db = new_db();
        run(&db, &["SET", "k", "v1"]);
        run(&db, &["SET", "k", "v2"]);
        assert_eq!(run(&db, &["GET", "k"]), bulk("v2"));
    }

    #[test]
    fn get_missing_key_is_null() {
        assert_eq!(run(&new_db(), &["GET", "nope"]), RespValue::Null);
    }

    #[test]
    fn wrong_argument_counts() {
        let db = new_db();
        assert_eq!(
            run(&db, &["SET", "k"]),
            error("ERR wrong number of arguments for 'set' command")
        );
        assert_eq!(
            run(&db, &["GET"]),
            error("ERR wrong number of arguments for 'get' command")
        );
        assert_eq!(
            run(&db, &["GET", "a", "b"]),
            error("ERR wrong number of arguments for 'get' command")
        );
    }

    #[test]
    fn set_rejects_bad_options() {
        let db = new_db();
        let invalid_expire = error("ERR invalid expire time in 'set' command");
        assert_eq!(run(&db, &["SET", "k", "v", "EX"]), invalid_expire);
        assert_eq!(run(&db, &["SET", "k", "v", "EX", "abc"]), invalid_expire);
        assert_eq!(run(&db, &["SET", "k", "v", "PX", "0"]), invalid_expire);
        assert_eq!(run(&db, &["SET", "k", "v", "PX", "-5"]), invalid_expire);
        assert_eq!(
            run(&db, &["SET", "k", "v", "EX", "18446744073709551615"]),
            invalid_expire
        );
        assert_eq!(
            run(&db, &["SET", "k", "v", "NX"]),
            error("ERR syntax error")
        );
    }

    #[test]
    fn rejected_set_does_not_store_the_key() {
        let db = new_db();
        run(&db, &["SET", "k", "v", "EX", "abc"]);
        assert_eq!(run(&db, &["GET", "k"]), RespValue::Null);
    }

    // --- expiry (paused clock: time only moves on `advance`) ---

    #[tokio::test(start_paused = true)]
    async fn px_expires() {
        let db = new_db();
        assert_eq!(run(&db, &["SET", "k", "v", "PX", "100"]), ok());
        tokio::time::advance(Duration::from_millis(99)).await;
        assert_eq!(run(&db, &["GET", "k"]), bulk("v"));
        tokio::time::advance(Duration::from_millis(2)).await;
        assert_eq!(run(&db, &["GET", "k"]), RespValue::Null);
    }

    #[tokio::test(start_paused = true)]
    async fn ex_expires() {
        let db = new_db();
        assert_eq!(run(&db, &["set", "k", "v", "ex", "2"]), ok());
        tokio::time::advance(Duration::from_millis(1999)).await;
        assert_eq!(run(&db, &["GET", "k"]), bulk("v"));
        tokio::time::advance(Duration::from_millis(2)).await;
        assert_eq!(run(&db, &["GET", "k"]), RespValue::Null);
    }

    #[tokio::test(start_paused = true)]
    async fn set_without_ttl_clears_old_ttl() {
        let db = new_db();
        run(&db, &["SET", "k", "v", "PX", "100"]);
        run(&db, &["SET", "k", "v2"]);
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(run(&db, &["GET", "k"]), bulk("v2"));
    }

    #[tokio::test(start_paused = true)]
    async fn last_expiry_option_wins() {
        let db = new_db();
        run(&db, &["SET", "k", "v", "EX", "100", "PX", "50"]);
        tokio::time::advance(Duration::from_millis(51)).await;
        assert_eq!(run(&db, &["GET", "k"]), RespValue::Null);
    }

    // --- binary safety ---

    fn run_bytes(db: &Mutex<DataManager>, args: &[&'static [u8]]) -> RespValue {
        let request = RespValue::Array(
            args.iter()
                .map(|&arg| RespValue::BulkString(Bytes::from_static(arg)))
                .collect(),
        );
        execute(request, db)
    }

    #[test]
    fn echo_is_binary_safe() {
        assert_eq!(
            run_bytes(&new_db(), &[b"ECHO", b"\xff\x00\xfe"]),
            RespValue::BulkString(Bytes::from_static(b"\xff\x00\xfe"))
        );
    }

    #[test]
    fn binary_keys_and_values_round_trip() {
        let db = new_db();
        assert_eq!(run_bytes(&db, &[b"SET", b"\xff", b"a\x00b"]), ok());
        assert_eq!(run_bytes(&db, &[b"SET", b"\xfe", b"other"]), ok());
        assert_eq!(
            run_bytes(&db, &[b"GET", b"\xff"]),
            RespValue::BulkString(Bytes::from_static(b"a\x00b"))
        );
    }

    #[test]
    fn non_utf8_expire_time_is_rejected() {
        assert_eq!(
            run_bytes(&new_db(), &[b"SET", b"k", b"v", b"PX", b"\xff"]),
            error("ERR invalid expire time in 'set' command")
        );
    }
}
