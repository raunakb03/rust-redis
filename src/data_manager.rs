use std::collections::HashMap;

use bytes::Bytes;
use tokio::time::Instant;

pub struct RedisData {
    val: Bytes,
    expiration: Option<Instant>
}

impl RedisData {
    pub fn new(val: Bytes) -> Self {
        Self {
            val,
            expiration: None
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
        Self { data: HashMap::new() }
    }

    pub fn insert(&mut self, key: Bytes, value: RedisData) {
        self.data.insert(key, value);
    }

    pub fn get(&mut self, key: &[u8]) -> Option<&Bytes> {
        let is_expired = if let Some(res) = self.data.get(key) {
            if let Some(timeout) = res.expiration {
                Instant::now() > timeout
            } else {
                false
            }
        } else {
            false
        };
        if is_expired {
            self.data.remove(key);
            return None;
        }
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
        assert_eq!(db.get(b"k"), Some(&Bytes::from("v")));
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
}
