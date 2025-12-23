//! API Key Pool Management
//!
//! Manages multiple API keys per provider with rotation strategies.

use crate::config::RotationStrategy;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A single API key with usage tracking
#[derive(Debug)]
pub struct ApiKey {
    /// The actual API key value
    value: String,

    /// Time until which this key is rate limited (if any)
    rate_limited_until: RwLock<Option<Instant>>,

    /// Total number of requests made with this key
    request_count: AtomicU64,

    /// Timestamp of last usage (for LRU strategy)
    last_used: AtomicU64,
}

impl ApiKey {
    /// Create a new API key
    pub fn new(value: String) -> Self {
        Self {
            value,
            rate_limited_until: RwLock::new(None),
            request_count: AtomicU64::new(0),
            last_used: AtomicU64::new(0),
        }
    }

    /// Get the key value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Check if this key is currently rate limited
    pub fn is_rate_limited(&self) -> bool {
        let guard = self.rate_limited_until.read();
        if let Some(until) = *guard {
            Instant::now() < until
        } else {
            false
        }
    }

    /// Get remaining rate limit duration
    pub fn rate_limit_remaining(&self) -> Option<Duration> {
        let guard = self.rate_limited_until.read();
        if let Some(until) = *guard {
            let now = Instant::now();
            if now < until {
                Some(until - now)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Mark this key as rate limited
    pub fn mark_rate_limited(&self, duration: Duration) {
        let mut guard = self.rate_limited_until.write();
        *guard = Some(Instant::now() + duration);
    }

    /// Clear rate limit status
    pub fn clear_rate_limit(&self) {
        let mut guard = self.rate_limited_until.write();
        *guard = None;
    }

    /// Record usage of this key
    pub fn record_usage(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        let now = Instant::now().elapsed().as_secs();
        self.last_used.store(now, Ordering::Relaxed);
    }

    /// Get the request count
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Get last used timestamp
    pub fn last_used(&self) -> u64 {
        self.last_used.load(Ordering::Relaxed)
    }
}

/// Pool of API keys with rotation support
#[derive(Debug)]
pub struct KeyPool {
    /// Provider name this pool belongs to
    provider: String,

    /// Available keys
    keys: Vec<ApiKey>,

    /// Current index for round-robin
    current_index: AtomicUsize,

    /// Rotation strategy
    strategy: RotationStrategy,
}

impl KeyPool {
    /// Create a new key pool
    pub fn new(provider: String, keys: Vec<String>, strategy: RotationStrategy) -> Self {
        Self {
            provider,
            keys: keys.into_iter().map(ApiKey::new).collect(),
            current_index: AtomicUsize::new(0),
            strategy,
        }
    }

    /// Get the provider name
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Get the number of keys in the pool
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Get the next available key based on rotation strategy
    pub fn get_key(&self) -> Option<&ApiKey> {
        if self.keys.is_empty() {
            return None;
        }

        match self.strategy {
            RotationStrategy::RoundRobin => self.get_round_robin(),
            RotationStrategy::LeastRecentlyUsed => self.get_lru(),
            RotationStrategy::Random => self.get_random(),
        }
    }

    /// Round-robin key selection
    fn get_round_robin(&self) -> Option<&ApiKey> {
        let len = self.keys.len();
        let mut attempts = 0;

        while attempts < len {
            let idx = self.current_index.fetch_add(1, Ordering::Relaxed) % len;
            let key = &self.keys[idx];

            if !key.is_rate_limited() {
                return Some(key);
            }

            attempts += 1;
        }

        // All keys are rate limited, return the one that will be available soonest
        self.get_soonest_available()
    }

    /// LRU key selection
    fn get_lru(&self) -> Option<&ApiKey> {
        self.keys
            .iter()
            .filter(|k| !k.is_rate_limited())
            .min_by_key(|k| k.last_used())
            .or_else(|| self.get_soonest_available())
    }

    /// Random key selection
    fn get_random(&self) -> Option<&ApiKey> {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let available: Vec<_> = self.keys.iter().filter(|k| !k.is_rate_limited()).collect();

        if available.is_empty() {
            return self.get_soonest_available();
        }

        // Simple pseudo-random selection
        let hasher = RandomState::new().build_hasher();
        let idx = hasher.finish() as usize % available.len();
        Some(available[idx])
    }

    /// Get the key that will be available soonest
    fn get_soonest_available(&self) -> Option<&ApiKey> {
        self.keys
            .iter()
            .min_by_key(|k| k.rate_limit_remaining().unwrap_or(Duration::ZERO))
    }

    /// Mark a specific key as rate limited
    pub fn mark_rate_limited(&self, key_value: &str, duration: Duration) {
        if let Some(key) = self.keys.iter().find(|k| k.value() == key_value) {
            key.mark_rate_limited(duration);
        }
    }

    /// Check if all keys are currently rate limited
    pub fn all_rate_limited(&self) -> bool {
        self.keys.iter().all(|k| k.is_rate_limited())
    }

    /// Get the minimum wait time until a key is available
    pub fn min_wait_time(&self) -> Option<Duration> {
        self.keys
            .iter()
            .filter_map(|k| k.rate_limit_remaining())
            .min()
    }

    /// Get statistics about the pool
    pub fn stats(&self) -> KeyPoolStats {
        let total = self.keys.len();
        let rate_limited = self.keys.iter().filter(|k| k.is_rate_limited()).count();
        let total_requests: u64 = self.keys.iter().map(|k| k.request_count()).sum();

        KeyPoolStats {
            total_keys: total,
            available_keys: total - rate_limited,
            rate_limited_keys: rate_limited,
            total_requests,
        }
    }
}

/// Statistics about a key pool
#[derive(Debug, Clone)]
pub struct KeyPoolStats {
    pub total_keys: usize,
    pub available_keys: usize,
    pub rate_limited_keys: usize,
    pub total_requests: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_rate_limiting() {
        let key = ApiKey::new("test-key".to_string());

        assert!(!key.is_rate_limited());

        key.mark_rate_limited(Duration::from_secs(60));
        assert!(key.is_rate_limited());

        key.clear_rate_limit();
        assert!(!key.is_rate_limited());
    }

    #[test]
    fn test_key_pool_round_robin() {
        let pool = KeyPool::new(
            "test".to_string(),
            vec!["key1".to_string(), "key2".to_string(), "key3".to_string()],
            RotationStrategy::RoundRobin,
        );

        let k1 = pool.get_key().unwrap();
        let k2 = pool.get_key().unwrap();
        let k3 = pool.get_key().unwrap();
        let k4 = pool.get_key().unwrap();

        // Should cycle through keys
        assert_eq!(k1.value(), "key1");
        assert_eq!(k2.value(), "key2");
        assert_eq!(k3.value(), "key3");
        assert_eq!(k4.value(), "key1");
    }

    #[test]
    fn test_key_pool_skips_rate_limited() {
        let pool = KeyPool::new(
            "test".to_string(),
            vec!["key1".to_string(), "key2".to_string()],
            RotationStrategy::RoundRobin,
        );

        // Rate limit the first key
        pool.mark_rate_limited("key1", Duration::from_secs(60));

        // Should skip key1
        let k = pool.get_key().unwrap();
        assert_eq!(k.value(), "key2");
    }

    #[test]
    fn test_all_rate_limited() {
        let pool = KeyPool::new(
            "test".to_string(),
            vec!["key1".to_string(), "key2".to_string()],
            RotationStrategy::RoundRobin,
        );

        assert!(!pool.all_rate_limited());

        pool.mark_rate_limited("key1", Duration::from_secs(60));
        pool.mark_rate_limited("key2", Duration::from_secs(60));

        assert!(pool.all_rate_limited());
    }
}
