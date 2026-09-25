use std::collections::HashMap;

use tokio::time::Instant;

pub struct RedisData {
    val: String,
    expiration: Option<Instant>
}

impl RedisData {
    pub fn new(val: String) -> Self {
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
    data: HashMap<String, RedisData>,
}

impl DataManager {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    pub fn insert(&mut self, key: String, value: RedisData) {
        self.data.insert(key, value);
    }

    pub fn get(&mut self, key: &str) -> Option<&String> {
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
