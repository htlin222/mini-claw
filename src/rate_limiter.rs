use std::collections::HashMap;
use tokio::time::Instant;

pub struct RateLimiter {
    entries: HashMap<i64, Instant>,
}

pub struct RateLimitResult {
    pub allowed: bool,
    pub retry_after_ms: Option<u64>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn check(&mut self, chat_id: i64, cooldown_ms: u64) -> RateLimitResult {
        let now = Instant::now();

        let Some(last) = self.entries.get(&chat_id) else {
            self.entries.insert(chat_id, now);
            return RateLimitResult {
                allowed: true,
                retry_after_ms: None,
            };
        };

        let elapsed_ms = now.duration_since(*last).as_millis() as u64;

        if elapsed_ms >= cooldown_ms {
            self.entries.insert(chat_id, now);
            RateLimitResult {
                allowed: true,
                retry_after_ms: None,
            }
        } else {
            RateLimitResult {
                allowed: false,
                retry_after_ms: Some(cooldown_ms - elapsed_ms),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_first_request_allowed() {
        tokio::time::pause();
        let mut limiter = RateLimiter::new();
        let result = limiter.check(123, 5000);
        assert!(result.allowed);
        assert!(result.retry_after_ms.is_none());
    }

    #[tokio::test]
    async fn test_second_request_within_cooldown_denied() {
        tokio::time::pause();
        let mut limiter = RateLimiter::new();
        limiter.check(123, 5000);
        let result = limiter.check(123, 5000);
        assert!(!result.allowed);
        assert!(result.retry_after_ms.is_some());
    }

    #[tokio::test]
    async fn test_request_after_cooldown_allowed() {
        tokio::time::pause();
        let mut limiter = RateLimiter::new();
        limiter.check(123, 5000);
        tokio::time::advance(std::time::Duration::from_millis(5001)).await;
        let result = limiter.check(123, 5000);
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_different_chats_independent() {
        tokio::time::pause();
        let mut limiter = RateLimiter::new();
        limiter.check(123, 5000);
        let result = limiter.check(456, 5000);
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_retry_after_decreases() {
        tokio::time::pause();
        let mut limiter = RateLimiter::new();
        limiter.check(123, 5000);

        tokio::time::advance(std::time::Duration::from_millis(2000)).await;
        let result = limiter.check(123, 5000);
        assert!(!result.allowed);
        let retry = result.retry_after_ms.unwrap();
        assert!(retry <= 3000);
        assert!(retry > 2000);
    }
}
