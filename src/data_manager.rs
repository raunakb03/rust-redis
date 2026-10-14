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

    pub fn get(&mut self, key: &[u8]) -> Option<&RedisDataType> {
        self.remove_if_expired(key);
        self.data.get(key).map(|res| &res.val)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn get_returns_inserted_value() {
        let mut db = DataManager::new();
        db.insert("k".into(), RedisData::new("v".into()));
        assert_eq!(db.get(b"k"), Some(&RedisDataType::Byte(Bytes::from("v"))));
        assert_eq!(db.get(b"missing"), None);
    }

    #[tokio::test(start_paused = true)]
    async fn expired_key_is_removed_on_read() {
        let mut db = DataManager::new();
        let expiration = Instant::now() + Duration::from_millis(10);
        db.insert(
            "k".into(),
            RedisData::new("v".into()).with_expiration(expiration),
        );
        assert!(db.get(b"k").is_some());

        tokio::time::advance(Duration::from_millis(11)).await;
        assert_eq!(db.get(b"k"), None);
        assert!(!db.data.contains_key(b"k".as_slice()));
    }

    #[test]
    fn insert_into_list_stores_elements_in_order() {
        let mut db = DataManager::new();
        assert_eq!(db.insert_into_list("k".into(), "a".into()), Ok(1));
        assert_eq!(db.insert_into_list("k".into(), "b".into()), Ok(2));
        assert_eq!(
            db.get(b"k"),
            Some(&RedisDataType::List(vec!["a".into(), "b".into()]))
        );
    }

    #[test]
    fn insert_into_list_rejects_string_key() {
        let mut db = DataManager::new();
        db.insert("k".into(), RedisData::new("v".into()));
        assert_eq!(
            db.insert_into_list("k".into(), "a".into()),
            Err(DataError::WrongType)
        );
    }
}
