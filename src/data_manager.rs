use std::collections::HashMap;

use bytes::Bytes;
use tokio::time::Instant;

#[derive(Debug, PartialEq)]
pub enum RedisDataType {
    Byte(Bytes),
    List(Vec<Bytes>),
}

pub struct RedisData {
    val: RedisDataType,
    expiration: Option<Instant>,
}

#[derive(Debug, PartialEq)]
pub enum DataError {
    WrongType,
}

fn handle_negative_index(idx: i64, len: usize) -> usize {
    if !idx.is_negative() {
        return idx as usize;
    }
    (len as i64 + idx).max(0) as usize
}

impl RedisData {
    pub fn new(val: Bytes) -> Self {
        Self {
            val: RedisDataType::Byte(val),
            expiration: None,
        }
    }

    pub fn with_expiration(mut self, expiration: Instant) -> Self {
        self.expiration = Some(expiration);
        self
    }
}

pub struct DataManager {
    data: HashMap<Bytes, RedisData>,
}

impl DataManager {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: Bytes, value: RedisData) {
        self.data.insert(key, value);
    }

    fn remove_if_expired(&mut self, key: &[u8]) {
        let is_expired = self
            .data
            .get(key)
            .and_then(|res| res.expiration)
            .is_some_and(|timeout| Instant::now() > timeout);
        if is_expired {
            self.data.remove(key);
        }
    }

    pub fn insert_into_list(&mut self, key: Bytes, value: Bytes) -> Result<usize, DataError> {
        self.remove_if_expired(&key);
        let data = self.data.entry(key).or_insert_with(|| RedisData {
            val: RedisDataType::List(Vec::new()),
            expiration: None,
        });
        match &mut data.val {
            RedisDataType::Byte(_) => Err(DataError::WrongType),
            RedisDataType::List(items) => {
                items.push(value);
                Ok(items.len())
            },
        }
    }


    pub fn get_from_list(&mut self, key: &[u8] ,start: i64, end: i64) -> Vec<Bytes> {
        let Some(RedisDataType::List(list)) = self.get(key) else {
            return Vec::new();
        };

        let start = if start.is_negative() {
            handle_negative_index(start, list.len())
        } else {
            start as usize
        };

        let end = if end.is_negative() {
            handle_negative_index(end, list.len())
        } else {
            end as usize
        };

        let end = end.min(list.len()-1);
        if start > end {
            return Vec::new();
        }

        list[start..=end].to_vec()
    }

    pub fn get(&mut self, key: &[u8]) -> Option<&RedisDataType> {
        self.remove_if_expired(key);
        self.data.get(key).map(|res| &res.val)
    }
}
