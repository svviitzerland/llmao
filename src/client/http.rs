//! HTTP Client
//!
//! Async HTTP client with retry, backoff, and rate limit handling.

use crate::client::rate_limiter::RateLimitTracker;
use crate::error::{LlmaoError, Result};
use backoff::ExponentialBackoff;
use futures::Stream;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// HTTP client with retry and rate limit handling
pub struct HttpClient {
    /// Inner reqwest client
    client: Client,

    /// Rate limit tracker
    rate_limiter: Arc<RateLimitTracker>,
}

impl HttpClient {
    /// Create a new HTTP client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout for long completions
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| LlmaoError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            rate_limiter: Arc::new(RateLimitTracker::new()),
        })
    }

    /// Get the rate limiter
    pub fn rate_limiter(&self) -> &Arc<RateLimitTracker> {
        &self.rate_limiter
    }

    /// Make a POST request with retry logic
    pub async fn post_with_retry<T, R>(
        &self,
        url: &str,
        body: &T,
        api_key: &str,
        extra_headers: Option<&HeaderMap>,
        provider: &str,
        max_retries: u32,
    ) -> Result<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| LlmaoError::Config(format!("Invalid API key format: {}", e)))?,
        );

        // Add extra headers
        if let Some(extra) = extra_headers {
            for (key, value) in extra {
                headers.insert(key.clone(), value.clone());
            }
        }

        let body_json = serde_json::to_string(body)?;

        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(120)),
            max_interval: Duration::from_secs(30),
            initial_interval: Duration::from_millis(500),
            multiplier: 2.0,
            ..Default::default()
        };

        let mut retries = 0;

        loop {
            // Check if we should wait due to rate limits
            if let Some(wait) = self.rate_limiter.should_wait(provider) {
                tokio::time::sleep(wait).await;
            }

            let response = self
                .client
                .post(url)
                .headers(headers.clone())
                .body(body_json.clone())
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();

                    // Update rate limit info from headers
                    self.rate_limiter.update_from_response(
                        provider,
                        resp.headers(),
                        None,
                        None,
                    );

                    if status.is_success() {
                        let body = resp.text().await?;
                        return serde_json::from_str(&body).map_err(|e| {
                            LlmaoError::Response(format!(
                                "Failed to parse response: {}. Body: {}",
                                e,
                                &body[..body.len().min(500)]
                            ))
                        });
                    }

                    let response_body = resp.text().await.unwrap_or_default();

                    // Handle rate limit
                    if RateLimitTracker::is_rate_limit_error(status.as_u16(), &response_body) {
                        // Parse headers from a new request since we consumed the response
                        retries += 1;
                        if retries > max_retries {
                            return Err(LlmaoError::RateLimited {
                                provider: provider.to_string(),
                                retry_after: None,
                            });
                        }

                        // Wait with exponential backoff
                        let wait = backoff.initial_interval * 2u32.pow(retries);
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    // Handle auth errors
                    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                        return Err(LlmaoError::Auth(format!(
                            "Authentication failed: {}",
                            response_body
                        )));
                    }

                    // Other errors
                    return Err(LlmaoError::Request(format!(
                        "Request failed with status {}: {}",
                        status, response_body
                    )));
                }
                Err(e) => {
                    retries += 1;
                    if retries > max_retries {
                        return Err(e.into());
                    }

                    // Retry on connection errors
                    if e.is_connect() || e.is_timeout() {
                        let wait = backoff.initial_interval * 2u32.pow(retries);
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    return Err(e.into());
                }
            }
        }
    }

    /// Make a streaming POST request
    pub async fn post_stream(
        &self,
        url: &str,
        body: &impl Serialize,
        api_key: &str,
        extra_headers: Option<&HeaderMap>,
        provider: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<bytes::Bytes>> + Send>>> {
        use async_stream::stream;
        use futures::StreamExt;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| LlmaoError::Config(format!("Invalid API key format: {}", e)))?,
        );

        // Add extra headers
        if let Some(extra) = extra_headers {
            for (key, value) in extra {
                headers.insert(key.clone(), value.clone());
            }
        }

        // Check rate limits
        if let Some(wait) = self.rate_limiter.should_wait(provider) {
            tokio::time::sleep(wait).await;
        }

        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(body)
            .send()
            .await?;

        let status = response.status();

        // Update rate limit info
        self.rate_limiter
            .update_from_response(provider, response.headers(), None, None);

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            if RateLimitTracker::is_rate_limit_error(status.as_u16(), &body) {
                return Err(LlmaoError::RateLimited {
                    provider: provider.to_string(),
                    retry_after: None,
                });
            }

            return Err(LlmaoError::Request(format!(
                "Streaming request failed with status {}: {}",
                status, body
            )));
        }

        // Convert to our stream type
        let mut byte_stream = response.bytes_stream();
        let s = stream! {
            while let Some(chunk) = byte_stream.next().await {
                yield chunk.map_err(LlmaoError::from);
            }
        };

        Ok(Box::pin(s))
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = HttpClient::new();
        assert!(client.is_ok());
    }
}

