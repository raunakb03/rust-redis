#[derive(Debug)]
pub enum RedisRes {
    String(usize, String),
    BulkString(usize, String),
    Array(Vec<RedisRes>),
    None,
}

fn convert_byte_to_int(byte: u8) -> u32 {
    byte as u32 - '0' as u32
}

pub fn parse(mut start: usize, end: usize, input: &[u8]) -> Option<(RedisRes, usize)> {
    if start > end {
        return None;
    }
    match input[start] {
        b'*' => {
            let mut num_eles = 0;
            start += 1;
            while input[start] != b'\r' {
                num_eles = num_eles*10 + convert_byte_to_int(input[start]);
                start += 1;
            }
            start += 2;
            let mut array_items = Vec::new();
            for _ in 0..num_eles {
                if let Some((parsed_item, new_start)) = parse(start, end, input) {
                    array_items.push(parsed_item);
                    start = new_start;
                }
            }
            Some((RedisRes::Array(array_items), start))
        },
        b'$' => {
            let mut len = 0;
            start += 1;
            while input[start] != b'\r' {
                len = len*10 + convert_byte_to_int(input[start]);
                start += 1;
            }
            start += 2;
            let string_bytes = &input[start..start + len as usize];
            let st = String::from_utf8_lossy(string_bytes).to_string();
            start = start + len as usize + 2;
            Some((RedisRes::BulkString(len as usize, st), start))
        },
        b':' => {
            todo!()
        },
        b'+' => {
            todo!()
        },
        _ => {
            let mut s = String::new();
            while input[start] != b'\r' {
                s.push(input[start] as char);
                start += 1;
            }
            start += 2;
            Some((RedisRes::String(s.len(), s), start))
        },
    }
}
