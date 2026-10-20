use std::collections::{HashMap, VecDeque};

use bytes::Bytes;
use tokio::time::Instant;

#[derive(Debug, PartialEq)]
pub enum RedisDataType {
    Byte(Bytes),
    List(VecDeque<Bytes>),
}

pub struct RedisData {
    val: RedisDataType,
    expiration: Option<Instant>,
}

enum ListDirection {
    Front,
    Back,
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

    fn insert_into_list(&mut self, key: Bytes, args: &[&Bytes], dir: ListDirection) -> Result<usize, DataError>{
        self.remove_if_expired(&key);
        let data = self.data.entry(key).or_insert_with(|| RedisData {
            val: RedisDataType::List(VecDeque::new()),
            expiration: None,
        });
        match &mut data.val {
            RedisDataType::Byte(_) => Err(DataError::WrongType),
            RedisDataType::List(items) => {
                for value in args {
                    match dir {
                        ListDirection::Front => items.push_front(Bytes::clone(value)),
                        ListDirection::Back => items.push_back(Bytes::clone(value)),
                    }
                    
                }
                Ok(items.len())
            },
        }
    }

    pub fn insert_into_list_back(&mut self, key: Bytes, args: &[&Bytes]) -> Result<usize, DataError> {
        self.insert_into_list(key, args, ListDirection::Back)
    }

    pub fn insert_into_list_front(&mut self, key: Bytes, args: &[&Bytes]) -> Result<usize, DataError> {
        self.insert_into_list(key, args, ListDirection::Front)
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

        list.range(start..=end).cloned().collect::<Vec<Bytes>>()
    }

    pub fn pop_from_list(&mut self, key: &[u8]) -> Option<Bytes> {
        let Some(RedisDataType::List(list)) = self.get_mut(key) else {
            return None;
        };

        let res = list.pop_front();

        if list.is_empty() {
            self.data.remove(key);
        }

        res
    }

    pub fn get(&mut self, key: &[u8]) -> Option<&RedisDataType> {
        self.remove_if_expired(key);
        self.data.get(key).map(|res| &res.val)
    }

    fn get_mut(&mut self, key: &[u8]) -> Option<&mut RedisDataType> {
        self.remove_if_expired(key);
        self.data.get_mut(key).map(|res| &mut res.val)
    }

    pub fn get_list_len(&mut self, key: &[u8]) -> Result<usize, DataError> {
        match self.get(key) {
            None => Ok(0),
            Some(RedisDataType::List(list)) => Ok(list.len()),
            Some(RedisDataType::Byte(_)) => Err(DataError::WrongType),
        }
    }
}
