use bytes::Bytes;

#[derive(Debug, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Bytes),
    Array(Vec<RespValue>),
    Null,
    NullArray,
}

impl RespValue {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            RespValue::SimpleString(s) => format!("+{s}\r\n").into_bytes(),
            RespValue::Error(msg) => format!("-{msg}\r\n").into_bytes(),
            RespValue::Integer(n) => format!(":{n}\r\n").into_bytes(),
            RespValue::BulkString(s) => {
                let mut out = format!("${}\r\n", s.len()).into_bytes();
                out.extend_from_slice(s);
                out.extend_from_slice(b"\r\n");
                out
            }
            RespValue::Array(items) => {
                let mut out = format!("*{}\r\n", items.len()).into_bytes();
                for item in items {
                    out.extend(item.encode());
                }
                out
            }
            RespValue::Null => b"$-1\r\n".to_vec(),
            RespValue::NullArray => b"*-1\r\n".to_vec(),
        }
    }
}

#[derive(Debug, PartialEq)]
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
pub const MAX_BULK_LEN: i64 = 512 * 1024 * 1024;

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
            Ok((
                RespValue::BulkString(Bytes::copy_from_slice(&input[pos..end])),
                end + 2,
            ))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn bulk(s: &str) -> RespValue {
        RespValue::BulkString(Bytes::copy_from_slice(s.as_bytes()))
    }

    fn is_invalid(result: Result<(RespValue, usize), ParserError>) -> bool {
        matches!(result, Err(ParserError::Invalid(_)))
    }

    // --- complete values ---

    #[test]
    fn parses_command_array() {
        assert_eq!(
            parse(b"*1\r\n$4\r\nPING\r\n"),
            Ok((RespValue::Array(vec![bulk("PING")]), 14))
        );
    }

    #[test]
    fn parses_array_with_mixed_types() {
        assert_eq!(
            parse(b"*3\r\n:1\r\n$2\r\nhi\r\n+OK\r\n"),
            Ok((
                RespValue::Array(vec![
                    RespValue::Integer(1),
                    bulk("hi"),
                    RespValue::SimpleString("OK".to_string()),
                ]),
                21
            ))
        );
    }

    #[test]
    fn parses_nested_array() {
        assert_eq!(
            parse(b"*1\r\n*1\r\n:5\r\n"),
            Ok((
                RespValue::Array(vec![RespValue::Array(vec![RespValue::Integer(5)])]),
                12
            ))
        );
    }

    #[test]
    fn parses_empty_array() {
        assert_eq!(parse(b"*0\r\n"), Ok((RespValue::Array(vec![]), 4)));
    }

    #[test]
    fn parses_empty_bulk_string() {
        assert_eq!(parse(b"$0\r\n\r\n"), Ok((bulk(""), 6)));
    }

    #[test]
    fn bulk_string_may_contain_crlf() {
        assert_eq!(parse(b"$4\r\na\r\nb\r\n"), Ok((bulk("a\r\nb"), 10)));
    }

    #[test]
    fn parses_simple_string() {
        assert_eq!(
            parse(b"+OK\r\n"),
            Ok((RespValue::SimpleString("OK".to_string()), 5))
        );
    }

    #[test]
    fn parses_integers() {
        assert_eq!(parse(b":42\r\n"), Ok((RespValue::Integer(42), 5)));
        assert_eq!(parse(b":-7\r\n"), Ok((RespValue::Integer(-7), 5)));
        assert_eq!(parse(b":+3\r\n"), Ok((RespValue::Integer(3), 5)));
    }

    #[test]
    fn parses_nulls() {
        assert_eq!(parse(b"$-1\r\n"), Ok((RespValue::Null, 5)));
        assert_eq!(parse(b"*-1\r\n"), Ok((RespValue::NullArray, 5)));
    }

    #[test]
    fn null_inside_array() {
        assert_eq!(
            parse(b"*2\r\n$3\r\nGET\r\n$-1\r\n"),
            Ok((RespValue::Array(vec![bulk("GET"), RespValue::Null]), 18))
        );
    }

    #[test]
    fn consumes_only_the_first_of_pipelined_values() {
        let input = b"*1\r\n$4\r\nPING\r\n*1\r\n$4\r\nPING\r\n";
        let (_, consumed) = parse(input).unwrap();
        assert_eq!(consumed, 14);
        assert_eq!(
            parse(&input[consumed..]),
            Ok((RespValue::Array(vec![bulk("PING")]), 14))
        );
    }

    // --- incomplete input ---

    #[test]
    fn incomplete_inputs() {
        let cases: &[&[u8]] = &[
            b"",
            b"*",
            b"*1",
            b"*1\r",
            b"*1\r\n",
            b"*1\r\n$4\r\nPI",
            b"*2\r\n$3\r\nGET\r\n",
            b"$5\r\nhel",
            b"$5\r\nhello",
            b"$5\r\nhello\r",
            b":42",
            b"+OK",
        ];
        for &input in cases {
            assert_eq!(
                parse(input),
                Err(ParserError::Incomplete),
                "input: {:?}",
                String::from_utf8_lossy(input)
            );
        }
    }

    // --- invalid input ---

    #[test]
    fn rejects_unknown_type_byte() {
        assert!(is_invalid(parse(b"A\r\n")));
        assert!(is_invalid(parse(b"PING\r\n")));
    }

    #[test]
    fn rejects_bad_lengths() {
        assert!(is_invalid(parse(b"$abc\r\n")));
        assert!(is_invalid(parse(b"$\r\n")));
        assert!(is_invalid(parse(b"$-2\r\n")));
        assert!(is_invalid(parse(b"*-5\r\n")));
        assert!(is_invalid(parse(b"*x\r\n")));
    }

    #[test]
    fn rejects_bulk_string_over_limit() {
        let input = format!("${}\r\n", MAX_BULK_LEN + 1);
        assert!(is_invalid(parse(input.as_bytes())));
        assert!(is_invalid(parse(b"$9223372036854775807\r\n")));
    }

    #[test]
    fn rejects_missing_crlf_after_bulk_string() {
        assert!(is_invalid(parse(b"$5\r\nhelloXY")));
    }

    #[test]
    fn rejects_bad_integers() {
        assert!(is_invalid(parse(b":abc\r\n")));
        assert!(is_invalid(parse(b":\r\n")));
    }

    #[test]
    fn invalid_element_makes_array_invalid() {
        assert!(is_invalid(parse(b"*2\r\n$3\r\nGET\r\n$x\r\n")));
    }

    // --- encode ---

    #[test]
    fn encodes_each_type() {
        assert_eq!(RespValue::SimpleString("OK".into()).encode(), b"+OK\r\n");
        assert_eq!(RespValue::Error("ERR bad".into()).encode(), b"-ERR bad\r\n");
        assert_eq!(RespValue::Integer(-3).encode(), b":-3\r\n");
        assert_eq!(bulk("hey").encode(), b"$3\r\nhey\r\n");
        assert_eq!(bulk("").encode(), b"$0\r\n\r\n");
        assert_eq!(RespValue::Null.encode(), b"$-1\r\n");
        assert_eq!(RespValue::NullArray.encode(), b"*-1\r\n");
        assert_eq!(
            RespValue::Array(vec![bulk("a"), RespValue::Integer(1)]).encode(),
            b"*2\r\n$1\r\na\r\n:1\r\n"
        );
    }

    #[test]
    fn bulk_string_length_is_in_bytes() {
        // "é" is 2 bytes in UTF-8.
        assert_eq!(bulk("é").encode(), "$2\r\né\r\n".as_bytes());
    }

    #[test]
    fn encode_then_parse_round_trips() {
        let values = [
            RespValue::SimpleString("OK".into()),
            RespValue::Integer(i64::MIN),
            bulk("hello world"),
            bulk("a\r\nb"),
            RespValue::Null,
            RespValue::NullArray,
            RespValue::Array(vec![
                bulk("SET"),
                RespValue::Array(vec![RespValue::Integer(1)]),
                RespValue::Null,
            ]),
        ];
        for value in values {
            let encoded = value.encode();
            assert_eq!(parse(&encoded), Ok((value, encoded.len())));
        }
    }

    #[test]
    fn binary_bulk_string_round_trips() {
        let value = RespValue::BulkString(Bytes::from_static(b"\xff\xfe"));
        let encoded = value.encode();
        assert_eq!(encoded, b"$2\r\n\xff\xfe\r\n");
        assert_eq!(parse(&encoded), Ok((value, encoded.len())));
    }
}
