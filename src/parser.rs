#[derive(Debug)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(String),
    Array(Vec<RespValue>),
    Null,
    NullArray,
}

#[derive(Debug)]
pub enum ParserError {
    Incomplete,
    Invalid(String),
}

fn parse_int(bytes: &[u8]) -> Result<i64, ParserError> {
    str::from_utf8(bytes)
        .ok()
        .and_then(|st| st.parse().ok())
        .ok_or_else(|| {
            ParserError::Invalid(format!(
                "invalid integer: {}",
                String::from_utf8_lossy(bytes)
            ))
        })
}

// Redis rejects bulk strings larger than 512 MB.
const MAX_BULK_LEN: i64 = 512 * 1024 * 1024;

pub fn parse(input: &[u8]) -> Result<(RespValue, usize), ParserError> {
    let Some(line_end) = input.windows(2).position(|w| w == b"\r\n") else {
        return Err(ParserError::Incomplete);
    };
    match input[0] {
        b'*' => {
            let num_eles = parse_int(&input[1..line_end])?;
            let mut pos = line_end + 2;
            if num_eles == -1 {
                return Ok((RespValue::NullArray, pos));
            }
            if num_eles < -1 {
                return Err(ParserError::Invalid(format!(
                    "invalid array length: {num_eles}"
                )));
            }
            let mut array_items = Vec::new();
            for _ in 0..num_eles {
                let (parsed_item, bytes_consumed) = parse(&input[pos..])?;
                array_items.push(parsed_item);
                pos += bytes_consumed;
            }
            Ok((RespValue::Array(array_items), pos))
        }
        b'$' => {
            let len = parse_int(&input[1..line_end])?;
            let pos = line_end + 2;
            if len == -1 {
                return Ok((RespValue::Null, pos));
            }
            if !(0..=MAX_BULK_LEN).contains(&len) {
                return Err(ParserError::Invalid(format!(
                    "invalid bulk string length: {len}"
                )));
            }
            let len = len as usize;
            let end = pos + len;
            if input.len() < end + 2 {
                return Err(ParserError::Incomplete);
            }
            if &input[end..end + 2] != b"\r\n" {
                return Err(ParserError::Invalid(format!(
                    "expected CRLF after bulk string, got {:?}",
                    String::from_utf8_lossy(&input[end..end + 2])
                )));
            }
            let st = String::from_utf8_lossy(&input[pos..end]).into_owned();
            Ok((RespValue::BulkString(st), end + 2))
        }
        b':' => {
            let val = parse_int(&input[1..line_end])?;
            Ok((RespValue::Integer(val), line_end + 2))
        }
        b'+' => {
            let res = String::from_utf8_lossy(&input[1..line_end]).into_owned();
            Ok((RespValue::SimpleString(res), line_end + 2))
        }
        _ => Err(ParserError::Invalid(format!(
            "unexpected type byte: {:?}",
            input[0] as char
        ))),
    }
}
