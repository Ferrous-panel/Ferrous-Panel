use crate::error::PanelError;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;

const MAX_FAILED_ATTEMPTS: usize = 5;
const WINDOW_SECS: i64 = 15 * 60;

/// Per-IP sliding-window rate limiter enforcing max 5 failed logins per 15 minutes.
#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    attempts: Mutex<HashMap<String, Vec<i64>>>,
}

impl LoginRateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rejects requests with HTTP 429 when an IP already reached 5 failures in the last 15 minutes.
    pub fn check(&self, ip: &str) -> Result<(), PanelError> {
        let now = Utc::now().timestamp();
        let cutoff = now - WINDOW_SECS;

        let mut guard = self
            .attempts
            .lock()
            .map_err(|_| PanelError::Internal("rate limiter state unavailable".to_string()))?;

        if let Some(timestamps) = guard.get_mut(ip) {
            timestamps.retain(|&ts| ts > cutoff);
            if timestamps.len() >= MAX_FAILED_ATTEMPTS {
                let oldest = timestamps.first().copied().unwrap_or(now);
                let retry_after = (oldest + WINDOW_SECS - now).max(1);
                return Err(PanelError::RateLimited {
                    retry_after_secs: retry_after,
                });
            }
            if timestamps.is_empty() {
                guard.remove(ip);
            }
        }
        Ok(())
    }

    /// Records a failed authentication attempt for the source IP.
    pub fn record_failure(&self, ip: &str) -> Result<(), PanelError> {
        let now = Utc::now().timestamp();
        let cutoff = now - WINDOW_SECS;

        let mut guard = self
            .attempts
            .lock()
            .map_err(|_| PanelError::Internal("rate limiter state unavailable".to_string()))?;

        let entry = guard.entry(ip.to_string()).or_default();
        entry.retain(|&ts| ts > cutoff);
        entry.push(now);
        Ok(())
    }

    /// Resets failure counter for an IP after a successful authentication.
    pub fn clear(&self, ip: &str) -> Result<(), PanelError> {
        let mut guard = self
            .attempts
            .lock()
            .map_err(|_| PanelError::Internal("rate limiter state unavailable".to_string()))?;
        guard.remove(ip);
        Ok(())
    }
}
