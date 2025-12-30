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
use config::{ConfigLoader, ProviderConfig};
use error::{LlmaoError, Result};
use router::{KeyPool, ModelRoute};

/// The main LLM client
pub struct LlmClient {
    /// Provider registry (metadata from registry.json)
    provider_registry: config::ProviderRegistry,

    /// Fallback provider configs (from user config with custom base_url)
    custom_providers: HashMap<String, ProviderConfig>,

    /// Expanded model configurations (provider/model -> config)
    #[allow(dead_code)] // Will be used for model-specific configuration lookups
    model_configs: HashMap<String, config::ModelConfig>,

    /// API key pools per provider
    key_pools: HashMap<String, KeyPool>,

    /// HTTP client
    http_client: HttpClient,
}

impl LlmClient {
    /// Create a new client with default configuration
    pub fn new() -> Result<Self> {
        let loader = ConfigLoader::new()?;
        Self::from_loader(loader)
    }

    /// Create a client with a custom config path
    pub fn with_config_path(path: &str) -> Result<Self> {
        let loader = ConfigLoader::from_path(path)?;
        Self::from_loader(loader)
    }

    /// Create a client from a config loader
    fn from_loader(loader: ConfigLoader) -> Result<Self> {
        let provider_registry = loader.provider_registry().clone();
        let user_config = loader.config().clone();

        // Expand user config into individual model configurations
        let mut model_configs = HashMap::new();
        let mut key_pools = HashMap::new();
        let mut custom_providers: HashMap<String, ProviderConfig> = HashMap::new();

        for (key, model_config) in user_config {
            // Check if key contains "/" (specific model) or not (provider-level)
            if key.contains('/') {
                // Specific model: "provider/model"
                let parts: Vec<&str> = key.splitn(2, '/').collect();
                let provider_name = parts[0];

                // Create key pool for this provider if not exists
                if !key_pools.contains_key(provider_name) && !model_config.keys.is_empty() {
                    key_pools.insert(
                        provider_name.to_string(),
                        KeyPool::new(
                            provider_name.to_string(),
                            model_config.keys.clone(),
                            model_config.rotation_strategy.clone(),
                        ),
                    );
                }

                // If this provider is not in registry and has a base_url, create a custom provider entry
                if !provider_registry.contains_key(provider_name) {
                    if let Some(base_url) = &model_config.base_url {
                        if !custom_providers.contains_key(provider_name) {
                            custom_providers.insert(
                                provider_name.to_string(),
                                ProviderConfig {
                                    base_url: base_url.clone(),
                                    api_key_env: None,
                                    api_keys_env: None,
                                    api_base_env: None,
                                    models: vec![],
                                    param_mappings: model_config.param_mappings.clone(),
                                    headers: model_config.headers.clone(),
                                    rate_limit: model_config.rate_limit.clone(),
                                    special_handling: Default::default(),
                                },
                            );
                        }
                    }
                }

                // Store model config
                model_configs.insert(key.clone(), model_config);
            } else {
                // Provider-level: expand to multiple models
                let provider_name = &key;

                // Create key pool for this provider
                if !model_config.keys.is_empty() {
                    key_pools.insert(
                        provider_name.clone(),
                        KeyPool::new(
                            provider_name.clone(),
                            model_config.keys.clone(),
                            model_config.rotation_strategy.clone(),
                        ),
                    );
                }

                // If this provider is not in registry and has a base_url, create a custom provider entry
                if !provider_registry.contains_key(provider_name) {
                    if let Some(base_url) = &model_config.base_url {
                        if !custom_providers.contains_key(provider_name) {
                            custom_providers.insert(
                                provider_name.clone(),
                                ProviderConfig {
                                    base_url: base_url.clone(),
                                    api_key_env: None,
                                    api_keys_env: None,
                                    api_base_env: None,
                                    models: model_config.models.clone(),
                                    param_mappings: model_config.param_mappings.clone(),
                                    headers: model_config.headers.clone(),
                                    rate_limit: model_config.rate_limit.clone(),
                                    special_handling: Default::default(),
                                },
                            );
                        }
                    }
                }

                // Expand each model
                for model_name in &model_config.models {
                    let model_key = format!("{}/{}", provider_name, model_name);
                    model_configs.insert(model_key, model_config.clone());
                }
            }
        }

        Ok(Self {
            provider_registry,
            custom_providers,
            model_configs,
            key_pools,
            http_client: HttpClient::new()?,
        })
    }

    /// Get a provider configuration from registry or custom providers
    fn get_provider(&self, name: &str) -> Result<&ProviderConfig> {
        // First check built-in registry
        if let Some(config) = self.provider_registry.get(name) {
            return Ok(config);
        }
        // Then check custom providers (from user config with base_url)
        self.custom_providers
            .get(name)
            .ok_or_else(|| LlmaoError::ProviderNotFound(name.to_string()))
    }

    /// Get the default model (first configured model)
    pub fn get_default_model(&self) -> Option<String> {
        self.model_configs.keys().next().cloned()
    }

    /// Get all configured models
    pub fn get_configured_models(&self) -> Vec<String> {
        self.model_configs.keys().cloned().collect()
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

    /// Make a streaming completion request
    /// Returns a vector of chunks (for Python compatibility - we collect all chunks in a blocking call,
    /// then Python iterates over them. For true streaming, we'd need async Python support.)
    pub async fn completion_stream(
        &self,
        model: &str,
        request: CompletionRequest,
    ) -> Result<Vec<api::StreamChunk>> {
        use futures::StreamExt;

        let route = ModelRoute::parse(model)?;
        let provider_config = self.get_provider(&route.provider)?;

        // Build request body with stream=true
        let mut body = serde_json::to_value(&request)?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "model".to_string(),
                serde_json::Value::String(route.model_id()),
            );
            obj.insert("stream".to_string(), serde_json::Value::Bool(true));
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

        // Get API key
        let api_key = self.get_api_key(&route.provider)?;

        // Make streaming request
        let mut stream = self
            .http_client
            .post_stream(&url, &body, &api_key, extra_headers.as_ref(), &route.provider)
            .await?;

        // Collect chunks
        let mut chunks = Vec::new();
        let mut buffer = String::new();

        while let Some(result) = stream.next().await {
            let bytes = result?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            // Process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if let Some(chunk) = api::parse_sse_line(&line)? {
                    chunks.push(chunk);
                }
            }
        }

        // Process remaining buffer
        if !buffer.trim().is_empty() {
            if let Some(chunk) = api::parse_sse_line(&buffer)? {
                chunks.push(chunk);
            }
        }

        Ok(chunks)
    }

    /// List available providers
    pub fn providers(&self) -> Vec<String> {
        self.provider_registry.keys().cloned().collect()
    }

    /// Get provider info
    pub fn provider_info(&self, name: &str) -> Option<ProviderInfo> {
        self.provider_registry.get(name).map(|p| ProviderInfo {
            name: name.to_string(),
            base_url: p.base_url.clone(),
            models: p.models.clone(),
            has_keys: self.key_pools.contains_key(name),
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
    #[pyo3(signature = (config_path=None, config=None))]
    fn new(config_path: Option<&str>, config: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let inner = if let Some(conf_dict) = config {
            // Load from dictionary
            let json_val = python_to_json(conf_dict.as_any())?;
            let providers_config: config::ProvidersConfig = serde_json::from_value(json_val)
                .map_err(|e| LlmaoError::Config(format!("Invalid config dict: {}", e)))?;

            let loader = ConfigLoader::from_config(providers_config)?;
            LlmClient::from_loader(loader)?
        } else if let Some(path) = config_path {
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
    #[pyo3(signature = (messages, model=None, temperature=None, max_tokens=None, stream=None, **kwargs))]
    fn completion(
        &self,
        py: Python<'_>,
        messages: &Bound<'_, PyList>,
        model: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        stream: Option<bool>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        // Resolve model: use provided or get default from config
        let model_str = if let Some(m) = model {
            m.to_string()
        } else {
            self.inner.get_default_model().ok_or_else(|| {
                LlmaoError::Config("No model specified and no models configured. Either pass model parameter or add models to config.".to_string())
            })?
        };

        // Convert Python messages to Rust
        let rust_messages = convert_messages(messages)?;

        // Build request
        let mut request = CompletionRequest::new(model_str.clone(), rust_messages);

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

        let response = self
            .runtime
            .block_on(async move { client.completion(&model_str, request).await })?;

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

            // Also expose tool_calls if present
            if let Some(tool_calls) = &choice.message.tool_calls {
                let tools_list = PyList::empty(py);
                for tool in tool_calls {
                    let tool_dict = PyDict::new(py);
                    tool_dict.set_item("id", &tool.id)?;
                    tool_dict.set_item("type", &tool.call_type)?;

                    let func_dict = PyDict::new(py);
                    func_dict.set_item("name", &tool.function.name)?;
                    func_dict.set_item("arguments", &tool.function.arguments)?;

                    tool_dict.set_item("function", func_dict)?;
                    tools_list.append(tool_dict)?;
                }
                message_dict.set_item("tool_calls", tools_list)?;
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

    /// List configured models
    fn models(&self) -> Vec<String> {
        self.inner.get_configured_models()
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

    /// Stream a completion request, yielding chunks as they arrive
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (messages, model=None, temperature=None, max_tokens=None, **kwargs))]
    fn stream_completion(
        &self,
        py: Python<'_>,
        messages: &Bound<'_, PyList>,
        model: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<StreamIterator>> {
        // Resolve model
        let model_str = if let Some(m) = model {
            m.to_string()
        } else {
            self.inner.get_default_model().ok_or_else(|| {
                LlmaoError::Config("No model specified and no models configured.".to_string())
            })?
        };

        // Convert Python messages to Rust
        let rust_messages = convert_messages(messages)?;

        // Build request
        let mut request = CompletionRequest::new(model_str.clone(), rust_messages);

        if let Some(temp) = temperature {
            request.temperature = Some(temp);
        }
        if let Some(max) = max_tokens {
            request.max_tokens = Some(max);
        }

        // Add extra kwargs
        if let Some(extra) = kwargs {
            for (key, value) in extra.iter() {
                let key_str: String = key.extract()?;
                let json_value = python_to_json(&value)?;
                request.extra.insert(key_str, json_value);
            }
        }

        // Run streaming completion synchronously
        let client = self.inner.clone();
        let chunks = self
            .runtime
            .block_on(async move { client.completion_stream(&model_str, request).await })?;

        // Create iterator with collected chunks
        Py::new(py, StreamIterator::new(chunks))
    }
}

/// Python iterator for streaming chunks
#[pyclass]
struct StreamIterator {
    chunks: Vec<api::StreamChunk>,
    index: usize,
}

impl StreamIterator {
    fn new(chunks: Vec<api::StreamChunk>) -> Self {
        Self { chunks, index: 0 }
    }
}

#[pymethods]
impl StreamIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> Option<Py<PyDict>> {
        if self.index >= self.chunks.len() {
            return None;
        }

        let chunk = &self.chunks[self.index];
        self.index += 1;

        let dict = PyDict::new(py);
        dict.set_item("id", &chunk.id).ok()?;
        dict.set_item("model", &chunk.model).ok()?;
        dict.set_item("created", chunk.created).ok()?;

        // Extract content from first choice delta
        if let Some(choice) = chunk.choices.first() {
            if let Some(content) = &choice.delta.content {
                dict.set_item("content", content).ok()?;
            }
            if let Some(role) = &choice.delta.role {
                dict.set_item("role", role).ok()?;
            }
            if let Some(reason) = &choice.finish_reason {
                dict.set_item("finish_reason", reason).ok()?;
            }
            dict.set_item("index", choice.index).ok()?;

            // Include tool call deltas if present
            if let Some(tool_calls) = &choice.delta.tool_calls {
                let tc_list = PyList::empty(py);
                for tc in tool_calls {
                    let tc_dict = PyDict::new(py);
                    tc_dict.set_item("index", tc.index).ok()?;
                    if let Some(id) = &tc.id {
                        tc_dict.set_item("id", id).ok()?;
                    }
                    if let Some(t) = &tc.call_type {
                        tc_dict.set_item("type", t).ok()?;
                    }
                    if let Some(func) = &tc.function {
                        let func_dict = PyDict::new(py);
                        if let Some(name) = &func.name {
                            func_dict.set_item("name", name).ok()?;
                        }
                        if let Some(args) = &func.arguments {
                            func_dict.set_item("arguments", args).ok()?;
                        }
                        tc_dict.set_item("function", func_dict).ok()?;
                    }
                    tc_list.append(tc_dict).ok()?;
                }
                dict.set_item("tool_calls", tc_list).ok()?;
            }
        }

        Some(dict.into())
    }
}

/// Convert Python list of message dicts to Rust Messages
fn convert_messages(messages: &Bound<'_, PyList>) -> PyResult<Vec<Message>> {
    use api::{FunctionCall, ToolCall};

    let mut result = Vec::new();

    for item in messages.iter() {
        let dict: &Bound<'_, PyDict> = item.cast()?;

        let role: String = dict
            .get_item("role")?
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>("missing 'role'"))?
            .extract()?;

        let content = if let Some(content_item) = dict.get_item("content")? {
            if content_item.is_none() {
                MessageContent::Text(String::new())
            } else if let Ok(s) = content_item.extract::<String>() {
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

        // Parse tool_calls if present
        let tool_calls: Option<Vec<ToolCall>> =
            if let Some(tc_list) = dict.get_item("tool_calls")? {
                if tc_list.is_none() {
                    None
                } else if let Ok(list) = tc_list.cast::<PyList>() {
                    let mut calls = Vec::new();
                    for tc in list.iter() {
                        if let Ok(tc_dict) = tc.cast::<PyDict>() {
                            let id: String = tc_dict
                                .get_item("id")?
                                .map(|v| v.extract().unwrap_or_default())
                                .unwrap_or_default();

                            let call_type: String = tc_dict
                                .get_item("type")?
                                .map(|v| v.extract().unwrap_or_else(|_| "function".to_string()))
                                .unwrap_or_else(|| "function".to_string());

                            // Parse function details
                            if let Some(func_obj) = tc_dict.get_item("function")? {
                                if let Ok(func_dict) = func_obj.cast::<PyDict>() {
                                    let name: String = func_dict
                                        .get_item("name")?
                                        .map(|v| v.extract().unwrap_or_default())
                                        .unwrap_or_default();

                                    let arguments: String = func_dict
                                        .get_item("arguments")?
                                        .map(|v| v.extract().unwrap_or_default())
                                        .unwrap_or_default();

                                    calls.push(ToolCall {
                                        id,
                                        call_type,
                                        function: FunctionCall { name, arguments },
                                    });
                                }
                            }
                        }
                    }
                    if calls.is_empty() {
                        None
                    } else {
                        Some(calls)
                    }
                } else {
                    None
                }
            } else {
                None
            };

        result.push(Message {
            role,
            content,
            reasoning: None,
            name,
            tool_calls,
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
#[pyo3(signature = (messages, model=None, temperature=None, max_tokens=None, **kwargs))]
fn completion(
    py: Python<'_>,
    messages: &Bound<'_, PyList>,
    model: Option<&str>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let client = PyLlmClient::new(None, None)?;
    client.completion(py, messages, model, temperature, max_tokens, None, kwargs)
}

/// Python module definition
#[pymodule]
fn _llmao(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLlmClient>()?;
    m.add_class::<StreamIterator>()?;
    m.add_function(wrap_pyfunction!(completion, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
