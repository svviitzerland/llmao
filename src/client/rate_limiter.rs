//! Rate Limit Tracking
//!
//! Tracks rate limits from API responses and manages wait times.

use parking_lot::RwLock;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tracks rate limit status for providers
#[derive(Debug, Default)]
pub struct RateLimitTracker {
    /// Per-provider rate limit info
    providers: RwLock<HashMap<String, ProviderRateLimit>>,
}

/// Rate limit info for a single provider
#[derive(Debug, Clone, Default)]
pub struct ProviderRateLimit {
    /// Remaining requests in current window
    pub requests_remaining: Option<u32>,

    /// Remaining tokens in current window
    pub tokens_remaining: Option<u32>,

    /// When the rate limit resets
    pub reset_at: Option<Instant>,

    /// Last known retry-after duration
    pub retry_after: Option<Duration>,
}

impl RateLimitTracker {
    /// Create a new rate limit tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Update rate limit info from response headers
    pub fn update_from_response(
        &self,
        provider: &str,
        headers: &HeaderMap,
        remaining_header: Option<&str>,
        reset_header: Option<&str>,
    ) {
        let mut providers = self.providers.write();
        let info = providers.entry(provider.to_string()).or_default();

        // Parse remaining requests header
        let remaining_key = remaining_header.unwrap_or("x-ratelimit-remaining-requests");
        if let Some(value) = headers.get(remaining_key) {
            if let Ok(s) = value.to_str() {
                if let Ok(n) = s.parse::<u32>() {
                    info.requests_remaining = Some(n);
                }
            }
        }

        // Parse reset header
        let reset_key = reset_header.unwrap_or("x-ratelimit-reset-requests");
        if let Some(value) = headers.get(reset_key) {
            if let Ok(s) = value.to_str() {
                // Try parsing as seconds
                if let Ok(secs) = s.parse::<u64>() {
                    info.reset_at = Some(Instant::now() + Duration::from_secs(secs));
                }
                // Try parsing as duration string (e.g., "1m30s")
                else if let Some(duration) = parse_duration_string(s) {
                    info.reset_at = Some(Instant::now() + duration);
                }
            }
        }
    }

    /// Update from a rate limit error response
    pub fn update_from_rate_limit_error(
        &self,
        provider: &str,
        headers: &HeaderMap,
        retry_after_header: Option<&str>,
    ) -> Duration {
        let mut providers = self.providers.write();
        let info = providers.entry(provider.to_string()).or_default();

        // Parse retry-after header
        let retry_key = retry_after_header.unwrap_or("retry-after");
        let retry_duration = if let Some(value) = headers.get(retry_key) {
            if let Ok(s) = value.to_str() {
                // Try parsing as seconds
                if let Ok(secs) = s.parse::<u64>() {
                    Some(Duration::from_secs(secs))
                }
                // Try parsing as duration string
                else {
                    parse_duration_string(s)
                }
            } else {
                None
            }
        } else {
            None
        };

        // Default to 60 seconds if no header
        let duration = retry_duration.unwrap_or(Duration::from_secs(60));
        info.retry_after = Some(duration);
        info.reset_at = Some(Instant::now() + duration);

        duration
    }

    /// Check if we should wait before making a request
    pub fn should_wait(&self, provider: &str) -> Option<Duration> {
        let providers = self.providers.read();

        if let Some(info) = providers.get(provider) {
            // Check if we're at zero remaining requests
            if info.requests_remaining == Some(0) {
                if let Some(reset_at) = info.reset_at {
                    let now = Instant::now();
                    if now < reset_at {
                        return Some(reset_at - now);
                    }
                }
            }

            // Check retry_after
            if let Some(retry_after) = info.retry_after {
                if let Some(reset_at) = info.reset_at {
                    let now = Instant::now();
                    if now < reset_at {
                        return Some(reset_at - now);
                    }
                } else {
                    // No reset time, use retry_after as fallback
                    return Some(retry_after);
                }
            }
        }

        None
    }

    /// Clear rate limit info for a provider
    pub fn clear(&self, provider: &str) {
        let mut providers = self.providers.write();
        providers.remove(provider);
    }

    /// Detect if a response indicates a rate limit error
    pub fn is_rate_limit_error(status: u16, body: &str) -> bool {
        // HTTP 429 Too Many Requests
        if status == 429 {
            return true;
        }

        // Some providers return 400 or 403 with rate limit messages
        let lower_body = body.to_lowercase();
        lower_body.contains("rate limit")
            || lower_body.contains("rate_limit")
            || lower_body.contains("too many requests")
            || lower_body.contains("quota exceeded")
    }
}

/// Parse a duration string like "1m30s" or "2h" into a Duration
fn parse_duration_string(s: &str) -> Option<Duration> {
    let s = s.trim();

    // Handle milliseconds first
    if let Some(stripped) = s.strip_suffix("ms") {
        return stripped.parse::<u64>().ok().map(Duration::from_millis);
    }

    // Try complex format first (e.g., "1m30s", "2h30m")
    if s.contains('h') || (s.contains('m') && s.contains('s')) {
        let mut total_secs = 0u64;
        let mut current_num = String::new();

        for c in s.chars() {
            if c.is_ascii_digit() {
                current_num.push(c);
            } else if !current_num.is_empty() {
                if let Ok(n) = current_num.parse::<u64>() {
                    match c {
                        'h' => total_secs += n * 3600,
                        'm' => total_secs += n * 60,
                        's' => total_secs += n,
                        _ => {}
                    }
                }
                current_num.clear();
            }
        }

        if total_secs > 0 {
            return Some(Duration::from_secs(total_secs));
        }
    }

    // Simple cases - single unit
    if let Some(stripped) = s.strip_suffix('s') {
        return stripped.parse::<f64>().ok().map(Duration::from_secs_f64);
    }
    if let Some(stripped) = s.strip_suffix('m') {
        return stripped
            .parse::<u64>()
            .ok()
            .map(|mins| Duration::from_secs(mins * 60));
    }
    if let Some(stripped) = s.strip_suffix('h') {
        return stripped
            .parse::<u64>()
            .ok()
            .map(|hours| Duration::from_secs(hours * 3600));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_string() {
        assert_eq!(parse_duration_string("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration_string("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration_string("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(
            parse_duration_string("1m30s"),
            Some(Duration::from_secs(90))
        );
        assert_eq!(
            parse_duration_string("500ms"),
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn test_is_rate_limit_error() {
        assert!(RateLimitTracker::is_rate_limit_error(429, ""));
        assert!(RateLimitTracker::is_rate_limit_error(
            400,
            "rate limit exceeded"
        ));
        assert!(RateLimitTracker::is_rate_limit_error(
            403,
            "Too Many Requests"
        ));
        assert!(!RateLimitTracker::is_rate_limit_error(200, "success"));
        assert!(!RateLimitTracker::is_rate_limit_error(
            500,
            "internal error"
        ));
    }

    #[test]
    fn test_rate_limit_tracker() {
        let tracker = RateLimitTracker::new();

        // Initially no wait
        assert!(tracker.should_wait("test").is_none());

        // Simulate rate limit
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "5".parse().unwrap());

        let duration = tracker.update_from_rate_limit_error("test", &headers, None);
        assert_eq!(duration, Duration::from_secs(5));

        // Should now need to wait
        assert!(tracker.should_wait("test").is_some());
    }
}
