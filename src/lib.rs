//! LLMAO - Lightweight LLM API Orchestrator
//!
//! A high-performance Python library written in Rust for unified LLM provider access
//! with intelligent rate limiting and key rotation.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;
use std::sync::Arc;

pub mod api;
pub mod client;
pub mod config;
pub mod error;
pub mod router;

use api::{CompletionRequest, CompletionResponse, Message, MessageContent};
use client::HttpClient;
use config::{ConfigLoader, ProviderConfig, ProvidersConfig};
use error::{LlmaoError, Result};
use router::{KeyPool, ModelRoute};

/// The main LLM client
pub struct LlmClient {
    /// Provider configurations
    config: ProvidersConfig,

    /// API key pools per provider
    key_pools: HashMap<String, KeyPool>,

    /// HTTP client
    http_client: HttpClient,
}

impl LlmClient {
    /// Create a new client with default configuration
    pub fn new() -> Result<Self> {
        let loader = ConfigLoader::new()?;
        Self::from_config(loader.into_config())
    }

    /// Create a client with a custom config path
    pub fn with_config_path(path: &str) -> Result<Self> {
        let loader = ConfigLoader::from_path(path)?;
        Self::from_config(loader.into_config())
    }

    /// Create a client from a config object
    fn from_config(config: ProvidersConfig) -> Result<Self> {
        let mut key_pools = HashMap::new();

        // Build key pools from provider configs and key_pools config
        for (name, provider) in &config.providers {
            let keys = provider.get_api_keys();
            if !keys.is_empty() {
                let strategy = config
                    .key_pools
                    .get(name)
                    .map(|p| p.rotation_strategy.clone())
                    .unwrap_or_default();

                key_pools.insert(name.clone(), KeyPool::new(name.clone(), keys, strategy));
            }
        }

        // Also check explicit key pools
        for (name, pool_config) in &config.key_pools {
            if !key_pools.contains_key(name) {
                let mut keys = Vec::new();

                // Load from env vars
                for env in &pool_config.keys_env {
                    if let Ok(key) = std::env::var(env) {
                        keys.push(key);
                    }
                }

                // Load raw keys
                keys.extend(pool_config.keys.clone());

                if !keys.is_empty() {
                    key_pools.insert(
                        name.clone(),
                        KeyPool::new(name.clone(), keys, pool_config.rotation_strategy.clone()),
                    );
                }
            }
        }

        Ok(Self {
            config,
            key_pools,
            http_client: HttpClient::new()?,
        })
    }

    /// Get a provider configuration
    fn get_provider(&self, name: &str) -> Result<&ProviderConfig> {
        self.config
            .providers
            .get(name)
            .ok_or_else(|| LlmaoError::ProviderNotFound(name.to_string()))
    }

    /// Get an API key for a provider
    fn get_api_key(&self, provider: &str) -> Result<String> {
        if let Some(pool) = self.key_pools.get(provider) {
            pool.get_key()
                .map(|k| k.value().to_string())
                .ok_or_else(|| LlmaoError::NoKeysAvailable(provider.to_string()))
        } else {
            // Try single env var from provider config
            let config = self.get_provider(provider)?;
            config
                .get_api_keys()
                .into_iter()
                .next()
                .ok_or_else(|| LlmaoError::NoKeysAvailable(provider.to_string()))
        }
    }

    /// Mark an API key as rate limited
    fn mark_key_rate_limited(&self, provider: &str, key: &str, duration: std::time::Duration) {
        if let Some(pool) = self.key_pools.get(provider) {
            pool.mark_rate_limited(key, duration);
        }
    }

    /// Make a completion request
    pub async fn completion(
        &self,
        model: &str,
        request: CompletionRequest,
    ) -> Result<CompletionResponse> {
        let route = ModelRoute::parse(model)?;
        let provider_config = self.get_provider(&route.provider)?;

        // Build request body
        let mut body = serde_json::to_value(&request)?;

        // Set the actual model name
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "model".to_string(),
                serde_json::Value::String(route.model_id()),
            );
        }

        // Apply parameter mappings
        provider_config.apply_param_mappings(&mut body);

        // Build URL
        let base_url = provider_config.get_base_url();
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        // Build extra headers
        let extra_headers = if provider_config.headers.is_empty() {
            None
        } else {
            let mut headers = reqwest::header::HeaderMap::new();
            for (key, value) in &provider_config.headers {
                if let (Ok(name), Ok(val)) = (
                    reqwest::header::HeaderName::try_from(key.as_str()),
                    reqwest::header::HeaderValue::from_str(value),
                ) {
                    headers.insert(name, val);
                }
            }
            Some(headers)
        };

        // Try with key rotation on rate limit
        let max_attempts = self
            .key_pools
            .get(&route.provider)
            .map(|p| p.len())
            .unwrap_or(1);
        let mut last_error = None;

        for _ in 0..max_attempts {
            let api_key = self.get_api_key(&route.provider)?;

            match self
                .http_client
                .post_with_retry::<_, CompletionResponse>(
                    &url,
                    &body,
                    &api_key,
                    extra_headers.as_ref(),
                    &route.provider,
                    3,
                )
                .await
            {
                Ok(response) => return Ok(response),
                Err(LlmaoError::RateLimited { retry_after, .. }) => {
                    // Mark this key as rate limited and try next
                    let duration = retry_after
                        .map(std::time::Duration::from_secs)
                        .unwrap_or(std::time::Duration::from_secs(60));
                    self.mark_key_rate_limited(&route.provider, &api_key, duration);
                    last_error = Some(LlmaoError::RateLimited {
                        provider: route.provider.clone(),
                        retry_after,
                    });
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(LlmaoError::NoKeysAvailable(route.provider)))
    }

    /// List available providers
    pub fn providers(&self) -> Vec<String> {
        self.config.providers.keys().cloned().collect()
    }

    /// Get provider info
    pub fn provider_info(&self, name: &str) -> Option<ProviderInfo> {
        self.config.providers.get(name).map(|p| ProviderInfo {
            name: name.to_string(),
            base_url: p.base_url.clone(),
            models: p.models.clone(),
            has_keys: !p.get_api_keys().is_empty(),
        })
    }
}

/// Provider information
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub has_keys: bool,
}

// =============================================================================
// Python Bindings
// =============================================================================

/// Python wrapper for the LLM client
#[pyclass(name = "LLMClient")]
struct PyLlmClient {
    inner: Arc<LlmClient>,
    runtime: tokio::runtime::Runtime,
}

#[pymethods]
impl PyLlmClient {
    /// Create a new client
    #[new]
    #[pyo3(signature = (config_path=None))]
    fn new(config_path: Option<&str>) -> PyResult<Self> {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let inner = if let Some(path) = config_path {
            LlmClient::with_config_path(path)?
        } else {
            LlmClient::new()?
        };

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| LlmaoError::Internal(format!("Failed to create runtime: {}", e)))?;

        Ok(Self {
            inner: Arc::new(inner),
            runtime,
        })
    }

    /// Make a completion request
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (model, messages, temperature=None, max_tokens=None, stream=None, **kwargs))]
    fn completion(
        &self,
        py: Python<'_>,
        model: &str,
        messages: &Bound<'_, PyList>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        stream: Option<bool>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        // Convert Python messages to Rust
        let rust_messages = convert_messages(messages)?;

        // Build request
        let mut request = CompletionRequest::new(model.to_string(), rust_messages);

        if let Some(temp) = temperature {
            request.temperature = Some(temp);
        }
        if let Some(max) = max_tokens {
            request.max_tokens = Some(max);
        }
        if let Some(s) = stream {
            request.stream = Some(s);
        }

        // Add extra kwargs
        if let Some(extra) = kwargs {
            for (key, value) in extra.iter() {
                let key_str: String = key.extract()?;
                let json_value = python_to_json(&value)?;
                request.extra.insert(key_str, json_value);
            }
        }

        // Run async completion
        let client = self.inner.clone();
        let model = model.to_string();

        let response = self
            .runtime
            .block_on(async move { client.completion(&model, request).await })?;

        // Convert response to Python dict
        let dict = PyDict::new(py);
        dict.set_item("id", &response.id)?;
        dict.set_item("object", &response.object)?;
        dict.set_item("created", response.created)?;
        dict.set_item("model", &response.model)?;

        // Convert choices
        let choices = PyList::empty(py);
        for choice in &response.choices {
            let choice_dict = PyDict::new(py);
            choice_dict.set_item("index", choice.index)?;
            choice_dict.set_item("finish_reason", &choice.finish_reason)?;

            let message_dict = PyDict::new(py);
            message_dict.set_item("role", &choice.message.role)?;

            // Use content, or fall back to reasoning if content is empty
            let content = {
                let main_content = choice.message.content.to_string_content();
                if main_content.is_empty() {
                    choice.message.reasoning.clone().unwrap_or_default()
                } else {
                    main_content
                }
            };
            message_dict.set_item("content", content)?;

            // Also expose reasoning if present
            if let Some(reasoning) = &choice.message.reasoning {
                message_dict.set_item("reasoning", reasoning)?;
            }
            choice_dict.set_item("message", message_dict)?;

            choices.append(choice_dict)?;
        }
        dict.set_item("choices", choices)?;

        // Convert usage
        if let Some(usage) = &response.usage {
            let usage_dict = PyDict::new(py);
            usage_dict.set_item("prompt_tokens", usage.prompt_tokens)?;
            usage_dict.set_item("completion_tokens", usage.completion_tokens)?;
            usage_dict.set_item("total_tokens", usage.total_tokens)?;
            dict.set_item("usage", usage_dict)?;
        }

        Ok(dict.into())
    }

    /// List available providers
    fn providers(&self) -> Vec<String> {
        self.inner.providers()
    }

    /// Get info about a provider
    fn provider_info(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.provider_info(name) {
            Some(info) => {
                let dict = PyDict::new(py);
                dict.set_item("name", &info.name)?;
                dict.set_item("base_url", &info.base_url)?;
                dict.set_item("models", &info.models)?;
                dict.set_item("has_keys", info.has_keys)?;
                Ok(Some(dict.into()))
            }
            None => Ok(None),
        }
    }
}

/// Convert Python list of message dicts to Rust Messages
fn convert_messages(messages: &Bound<'_, PyList>) -> PyResult<Vec<Message>> {
    let mut result = Vec::new();

    for item in messages.iter() {
        let dict: &Bound<'_, PyDict> = item.cast()?;

        let role: String = dict
            .get_item("role")?
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>("missing 'role'"))?
            .extract()?;

        let content = if let Some(content_item) = dict.get_item("content")? {
            if let Ok(s) = content_item.extract::<String>() {
                MessageContent::Text(s)
            } else {
                // TODO: Handle content arrays for multimodal
                MessageContent::Text(content_item.str()?.to_string())
            }
        } else {
            MessageContent::Text(String::new())
        };

        let name: Option<String> = dict.get_item("name")?.and_then(|v| v.extract().ok());

        let tool_call_id: Option<String> = dict
            .get_item("tool_call_id")?
            .and_then(|v| v.extract().ok());

        result.push(Message {
            role,
            content,
            reasoning: None,
            name,
            tool_calls: None, // TODO: Handle tool calls
            tool_call_id,
        });
    }

    Ok(result)
}

/// Convert Python object to serde_json::Value
fn python_to_json(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        Ok(serde_json::Value::Null)
    } else if let Ok(b) = obj.extract::<bool>() {
        Ok(serde_json::Value::Bool(b))
    } else if let Ok(i) = obj.extract::<i64>() {
        Ok(serde_json::Value::Number(i.into()))
    } else if let Ok(f) = obj.extract::<f64>() {
        Ok(serde_json::json!(f))
    } else if let Ok(s) = obj.extract::<String>() {
        Ok(serde_json::Value::String(s))
    } else if let Ok(list) = obj.cast::<PyList>() {
        let vec: std::result::Result<Vec<_>, _> =
            list.iter().map(|item| python_to_json(&item)).collect();
        Ok(serde_json::Value::Array(vec?))
    } else if let Ok(dict) = obj.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key_str: String = key.extract()?;
            map.insert(key_str, python_to_json(&value)?);
        }
        Ok(serde_json::Value::Object(map))
    } else {
        // Fallback to string representation
        Ok(serde_json::Value::String(obj.str()?.to_string()))
    }
}

/// Convenience function for quick completions
#[pyfunction]
#[pyo3(signature = (model, messages, temperature=None, max_tokens=None, **kwargs))]
fn completion(
    py: Python<'_>,
    model: &str,
    messages: &Bound<'_, PyList>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let client = PyLlmClient::new(None)?;
    client.completion(py, model, messages, temperature, max_tokens, None, kwargs)
}

/// Python module definition
#[pymodule]
fn _llmao(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLlmClient>()?;
    m.add_function(wrap_pyfunction!(completion, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
