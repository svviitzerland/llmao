//! Provider Configuration
//!
//! Defines the configuration schema for LLM providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    /// Provider configurations keyed by provider name
    pub providers: HashMap<String, ProviderConfig>,

    /// Optional key pool configurations
    #[serde(default)]
    pub key_pools: HashMap<String, KeyPoolConfig>,
}

/// Configuration for a single LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Base URL for the API
    pub base_url: String,

    /// Environment variable name for the API key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,

    /// Optional list of environment variables for multiple keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys_env: Option<Vec<String>>,

    /// Optional environment variable for custom base URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_env: Option<String>,

    /// List of supported models (for documentation/validation)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,

    /// Parameter name mappings (e.g., max_completion_tokens -> max_tokens)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub param_mappings: HashMap<String, String>,

    /// Additional headers to send with requests
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// Rate limit configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,

    /// Special handling flags
    #[serde(default, skip_serializing_if = "SpecialHandling::is_default")]
    pub special_handling: SpecialHandling,
}

/// Rate limit configuration for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<u32>,

    /// Maximum tokens per minute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_minute: Option<u32>,

    /// Custom header name for retry-after (default: "retry-after")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_header: Option<String>,

    /// Custom header for remaining requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_requests_header: Option<String>,

    /// Custom header for rate limit reset time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_header: Option<String>,
}

/// Key pool configuration for multi-key support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPoolConfig {
    /// List of environment variable names containing API keys
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys_env: Vec<String>,

    /// List of raw API keys (alternative to keys_env)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,

    /// Rotation strategy
    #[serde(default)]
    pub rotation_strategy: RotationStrategy,
}

/// Strategy for rotating API keys
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RotationStrategy {
    /// Rotate through keys sequentially
    #[default]
    RoundRobin,

    /// Use the least recently used key
    LeastRecentlyUsed,

    /// Random selection
    Random,
}

/// Special handling flags for provider-specific quirks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpecialHandling {
    /// Convert content list to string (for providers that don't support content arrays)
    #[serde(default)]
    pub convert_content_list_to_string: bool,

    /// Add empty text field to assistant messages with tool calls
    #[serde(default)]
    pub add_text_to_tool_calls: bool,

    /// Use legacy completion endpoint instead of chat
    #[serde(default)]
    pub use_legacy_completions: bool,
}

impl SpecialHandling {
    pub fn is_default(&self) -> bool {
        !self.convert_content_list_to_string
            && !self.add_text_to_tool_calls
            && !self.use_legacy_completions
    }
}

impl ProviderConfig {
    /// Get the effective base URL (from env var if configured, otherwise default)
    pub fn get_base_url(&self) -> String {
        if let Some(env_var) = &self.api_base_env {
            if let Ok(url) = std::env::var(env_var) {
                return url;
            }
        }
        self.base_url.clone()
    }

    /// Get all API keys for this provider
    pub fn get_api_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();

        // First, try the single key env var
        if let Some(env_var) = &self.api_key_env {
            if let Ok(key) = std::env::var(env_var) {
                keys.push(key);
            }
        }

        // Then, add any additional keys from the list
        if let Some(env_vars) = &self.api_keys_env {
            for env_var in env_vars {
                if let Ok(key) = std::env::var(env_var) {
                    if !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            }
        }

        keys
    }

    /// Apply parameter mappings to a request body
    pub fn apply_param_mappings(&self, params: &mut serde_json::Value) {
        if let Some(obj) = params.as_object_mut() {
            for (from, to) in &self.param_mappings {
                if let Some(value) = obj.remove(from) {
                    obj.insert(to.clone(), value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_provider_config() {
        let json = r#"{
            "base_url": "https://api.example.com/v1",
            "api_key_env": "EXAMPLE_API_KEY",
            "models": ["model-a", "model-b"],
            "param_mappings": {
                "max_completion_tokens": "max_tokens"
            }
        }"#;

        let config: ProviderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.base_url, "https://api.example.com/v1");
        assert_eq!(config.api_key_env, Some("EXAMPLE_API_KEY".to_string()));
        assert_eq!(config.models.len(), 2);
        assert_eq!(
            config.param_mappings.get("max_completion_tokens"),
            Some(&"max_tokens".to_string())
        );
    }

    #[test]
    fn test_apply_param_mappings() {
        let config = ProviderConfig {
            base_url: "https://api.example.com".to_string(),
            api_key_env: None,
            api_keys_env: None,
            api_base_env: None,
            models: vec![],
            param_mappings: [(
                "max_completion_tokens".to_string(),
                "max_tokens".to_string(),
            )]
            .into_iter()
            .collect(),
            headers: HashMap::new(),
            rate_limit: None,
            special_handling: SpecialHandling::default(),
        };

        let mut params = serde_json::json!({
            "model": "gpt-4",
            "max_completion_tokens": 1000
        });

        config.apply_param_mappings(&mut params);

        assert!(params.get("max_completion_tokens").is_none());
        assert_eq!(params.get("max_tokens").unwrap(), 1000);
    }
}
