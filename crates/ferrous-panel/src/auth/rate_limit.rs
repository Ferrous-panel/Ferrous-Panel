use crate::config::Config;
use crate::error::PanelError;
use actix_web::HttpRequest;
use chrono::Utc;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

const MAX_FAILED_ATTEMPTS: usize = 5;
const WINDOW_SECS: i64 = 15 * 60;
const MAX_TRACKED_IPS: usize = 4096;

/// Extracts the real client IP address, honoring forwarded headers (`CF-Connecting-IP`,
/// `X-Real-IP`, `X-Forwarded-For`) ONLY when the direct TCP peer is explicitly listed
/// in `Config::trusted_proxies`.
#[must_use]
pub fn extract_client_ip(req: &HttpRequest, config: &Config) -> String {
    let peer_ip = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    if !config.is_trusted_proxy(&peer_ip) {
        return peer_ip;
    }

    for hdr_name in ["cf-connecting-ip", "x-real-ip"] {
        if let Some(raw) = req
            .headers()
            .get(hdr_name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
        {
            if let Ok(parsed) = raw.parse::<IpAddr>() {
                return parsed.to_string();
            }
        }
    }

    if let Some(xff) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = xff.split(',').next().map(str::trim) {
            if let Ok(parsed) = first.parse::<IpAddr>() {
                return parsed.to_string();
            }
        }
    }

    peer_ip
}

/// Bounded per-IP sliding-window rate limiter enforcing max 5 failed logins per 15 minutes
/// while capping in-memory tracked IPs to prevent memory exhaustion under distributed attacks.
#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    attempts: Mutex<HashMap<String, Vec<i64>>>,
}

impl LoginRateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn prune_locked(map: &mut HashMap<String, Vec<i64>>, cutoff: i64) {
        map.retain(|_, timestamps| {
            timestamps.retain(|&ts| ts > cutoff);
            !timestamps.is_empty()
        });
        while map.len() >= MAX_TRACKED_IPS {
            let oldest_key = map
                .iter()
                .min_by_key(|(_, ts)| ts.last().copied().unwrap_or(0))
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest_key {
                map.remove(&key);
            } else {
                break;
            }
        }
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

        if !guard.contains_key(ip) && guard.len() >= MAX_TRACKED_IPS {
            Self::prune_locked(&mut guard, cutoff);
        }

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
