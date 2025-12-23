//! LLMAO Error Types
//!
//! Comprehensive error handling for the LLM client library.

use pyo3::exceptions::{PyConnectionError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::fmt;

/// Main error type for LLMAO operations
#[derive(Debug)]
pub enum LlmaoError {
    /// Configuration errors (invalid JSON, missing fields, etc.)
    Config(String),

    /// Provider not found in registry
    ProviderNotFound(String),

    /// Model not supported by provider
    ModelNotSupported { provider: String, model: String },

    /// No API keys available (all rate limited or none configured)
    NoKeysAvailable(String),

    /// Rate limit exceeded
    RateLimited {
        provider: String,
        retry_after: Option<u64>,
    },

    /// HTTP request failed
    Request(String),

    /// Response parsing failed
    Response(String),

    /// Streaming error
    Stream(String),

    /// Authentication failed
    Auth(String),

    /// Timeout
    Timeout(String),

    /// Generic internal error
    Internal(String),
}

impl fmt::Display for LlmaoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmaoError::Config(msg) => write!(f, "Configuration error: {}", msg),
            LlmaoError::ProviderNotFound(name) => {
                write!(
                    f,
                    "Provider '{}' not found. Add it to your config with `base_url`, or submit a PR: https://github.com/svviitzerland/LLMAO",
                    name
                )
            }
            LlmaoError::ModelNotSupported { provider, model } => {
                write!(
                    f,
                    "Model '{}' not supported by provider '{}'. Check available models at provider's documentation.",
                    model, provider
                )
            }
            LlmaoError::NoKeysAvailable(provider) => {
                write!(
                    f,
                    "No API keys available for '{}'. Set {}_API_KEY env var or add keys in config.json",
                    provider,
                    provider.to_uppercase()
                )
            }
            LlmaoError::RateLimited {
                provider,
                retry_after,
            } => {
                if let Some(seconds) = retry_after {
                    write!(
                        f,
                        "Rate limited by '{}', retry after {} seconds",
                        provider, seconds
                    )
                } else {
                    write!(
                        f,
                        "Rate limited by '{}'. Consider adding more API keys for rotation.",
                        provider
                    )
                }
            }
            LlmaoError::Request(msg) => write!(f, "Request failed: {}", msg),
            LlmaoError::Response(msg) => write!(f, "Response error: {}", msg),
            LlmaoError::Stream(msg) => write!(f, "Streaming error: {}", msg),
            LlmaoError::Auth(msg) => {
                write!(f, "Authentication failed: {}. Check your API key.", msg)
            }
            LlmaoError::Timeout(msg) => write!(f, "Request timeout: {}", msg),
            LlmaoError::Internal(msg) => {
                write!(
                    f,
                    "Internal error: {}. Please report this issue: https://github.com/svviitzerland/LLMAO/issues",
                    msg
                )
            }
        }
    }
}

impl std::error::Error for LlmaoError {}

impl From<reqwest::Error> for LlmaoError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            LlmaoError::Timeout(err.to_string())
        } else if err.is_connect() {
            LlmaoError::Request(format!("Connection failed: {}", err))
        } else if err.is_decode() {
            LlmaoError::Response(format!("Failed to decode response: {}", err))
        } else {
            LlmaoError::Request(err.to_string())
        }
    }
}

impl From<serde_json::Error> for LlmaoError {
    fn from(err: serde_json::Error) -> Self {
        LlmaoError::Response(format!("JSON parsing error: {}", err))
    }
}

impl From<std::io::Error> for LlmaoError {
    fn from(err: std::io::Error) -> Self {
        LlmaoError::Config(format!("IO error: {}", err))
    }
}

impl From<LlmaoError> for PyErr {
    fn from(err: LlmaoError) -> PyErr {
        let msg = err.to_string();
        match &err {
            LlmaoError::Config(_) => PyValueError::new_err(msg),
            LlmaoError::ProviderNotFound(_) => PyValueError::new_err(msg),
            LlmaoError::ModelNotSupported { .. } => PyValueError::new_err(msg),
            LlmaoError::NoKeysAvailable(_) => PyRuntimeError::new_err(msg),
            LlmaoError::RateLimited { .. } => PyRuntimeError::new_err(msg),
            LlmaoError::Request(_) => PyConnectionError::new_err(msg),
            LlmaoError::Response(_) => PyRuntimeError::new_err(msg),
            LlmaoError::Stream(_) => PyRuntimeError::new_err(msg),
            LlmaoError::Auth(_) => PyRuntimeError::new_err(format!("Auth error: {}", msg)),
            LlmaoError::Timeout(_) => PyConnectionError::new_err(msg),
            LlmaoError::Internal(_) => PyRuntimeError::new_err(msg),
        }
    }
}

/// Result type alias for LLMAO operations
pub type Result<T> = std::result::Result<T, LlmaoError>;
