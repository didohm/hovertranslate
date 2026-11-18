use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A small in-memory cache of recent translations.
///
/// It is deliberately *not* persisted: the product requirement is that
/// translated text and captured content never touch the disk.
pub struct TranslationCache {
    entries: HashMap<String, Entry>,
    order: Vec<String>,
    capacity: usize,
    ttl: Duration,
}

struct Entry {
    value: String,
    stored_at: Instant,
}

impl TranslationCache {
    pub fn new(capacity: usize, ttl_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
            capacity: capacity.max(1),
            ttl: Duration::from_secs(ttl_secs.max(1)),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        let expired = match self.entries.get(key) {
            Some(entry) => entry.stored_at.elapsed() > self.ttl,
            None => return None,
        };
        if expired {
            self.remove(key);
            return None;
        }
        self.order.retain(|k| k != key);
        self.order.push(key.to_string());
        self.entries.get(key).map(|e| e.value.clone())
    }

    pub fn put(&mut self, key: String, value: String) {
        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self.order.first().cloned() {
                self.remove(&oldest);
            }
        }
        if self.entries.insert(
            key.clone(),
            Entry {
                value,
                stored_at: Instant::now(),
            },
        )
        .is_some()
        {
            self.order.retain(|k| k != &key);
        }
        self.order.push(key);
    }

    fn remove(&mut self, key: &str) {
        self.entries.remove(key);
        self.order.retain(|k| k != key);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
