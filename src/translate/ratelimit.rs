use std::time::{Duration, Instant};

/// Token bucket rate limiter: keeps the app from burning through a free
/// provider's daily allowance or a paid provider's quota.
pub struct RateLimiter {
    capacity: f64,
    tokens: f64,
    refill_per_second: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        let capacity = requests_per_minute.max(1) as f64;
        Self {
            capacity,
            tokens: capacity,
            refill_per_second: capacity / 60.0,
            last: Instant::now(),
        }
    }

    /// Re-create the limiter with a new budget (settings changed).
    pub fn resize(&mut self, requests_per_minute: u32) {
        let capacity = requests_per_minute.max(1) as f64;
        self.capacity = capacity;
        self.tokens = self.tokens.min(capacity);
        self.refill_per_second = capacity / 60.0;
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_second).min(self.capacity);
        self.last = now;
    }

    /// Try to take one slot. Returns how long to wait when the budget is spent.
    pub fn try_acquire(&mut self) -> Result<(), Duration> {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let missing = 1.0 - self.tokens;
            let wait = Duration::from_secs_f64((missing / self.refill_per_second).max(0.05));
            Err(wait)
        }
    }

    pub fn remaining(&mut self) -> f64 {
        self.refill();
        self.tokens.floor().max(0.0)
    }
}
