use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

pub struct LoginRateLimiter {
    attempts: Mutex<HashMap<String, (u32, Instant)>>,
}

impl LoginRateLimiter {
    fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, ip: &str) -> Result<(), u64> {
        let mut attempts = self.attempts.lock().unwrap();
        let now = Instant::now();
        let entry = attempts.entry(ip.to_string()).or_insert((0, now));

        if now.duration_since(entry.1).as_secs() >= 60 {
            *entry = (0, now);
        }

        entry.0 += 1;
        if entry.0 > 5 {
            let elapsed = now.duration_since(entry.1).as_secs();
            let retry_after = 60_u64.saturating_sub(elapsed);
            Err(retry_after)
        } else {
            Ok(())
        }
    }
}

pub static LOGIN_RATE_LIMITER: LazyLock<LoginRateLimiter> = LazyLock::new(LoginRateLimiter::new);
