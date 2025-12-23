# LLMAO

Lightweight LLM API Orchestrator - A fast, multi-provider LLM client with rate limiting and key rotation.

## Features

- **High Performance**: Core logic written in Rust with Python bindings via PyO3
- **Multi-Provider Support**: 20+ providers pre-configured (OpenAI, Anthropic, Groq, Cerebras, etc.)
- **Key Rotation**: Automatic rotation between multiple API keys when rate limited
- **Rate Limit Handling**: Intelligent detection and handling of rate limits with exponential backoff
- **Simple API**: Clean `provider/model` routing format
- **Extensible**: Easy to add new providers via JSON configuration

## Installation

```bash
pip install llmao
```

Or build from source:

```bash
# Requires Rust and maturin
pip install maturin
maturin develop
```

## Quick Start

```python
from llmao import completion

# Simple completion
response = completion(
    model="openai/gpt-4",
    messages=[{"role": "user", "content": "Hello!"}]
)
print(response["choices"][0]["message"]["content"])
```

## Usage

### Basic Usage

```python
from llmao import LLMClient

# Create client (loads .env automatically)
client = LLMClient()

# Make a completion request
response = client.completion(
    model="groq/llama-3.1-70b-versatile",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "What is the capital of France?"}
    ],
    temperature=0.7,
    max_tokens=100
)

print(response["choices"][0]["message"]["content"])
```

### Model Routing Format

Models are specified in the format `provider/model` or `provider/model/variant`:

```python
# Standard format
client.completion(model="openai/gpt-4o", messages=[...])
client.completion(model="anthropic/claude-3-5-sonnet-20241022", messages=[...])
client.completion(model="groq/llama-3.3-70b-versatile", messages=[...])

# With variant (e.g., Azure deployments)
client.completion(model="azure/gpt-4/my-deployment", messages=[...])
```

### Available Providers

List all configured providers:

```python
client = LLMClient()
print(client.providers())
# ['openai', 'anthropic', 'groq', 'cerebras', 'together', ...]

# Get provider details
info = client.provider_info("openai")
print(info)
# {'name': 'openai', 'base_url': 'https://api.openai.com/v1', 'models': [...], 'has_keys': True}
```

### Environment Variables

Set API keys via environment variables:

```bash
export OPENAI_API_KEY="sk-..."
export GROQ_API_KEY="gsk_..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

Or use a `.env` file (automatically loaded):

```
OPENAI_API_KEY=sk-...
GROQ_API_KEY=gsk_...
```

## Configuration

### Custom Providers

Create a `providers.json` or `llmao.json` in your project directory:

```json
{
  "providers": {
    "my_provider": {
      "base_url": "https://api.my-provider.com/v1",
      "api_key_env": "MY_PROVIDER_API_KEY",
      "models": ["model-a", "model-b"],
      "param_mappings": {
        "max_completion_tokens": "max_tokens"
      }
    }
  }
}
```

Configuration is loaded from (in order, later overrides earlier):
1. Built-in defaults
2. `$LLMAO_PROVIDERS_PATH` environment variable
3. `./providers.json` or `./llmao.json`
4. `~/.config/llmao/providers.json`
5. `~/.llmao/providers.json`

### Multi-Key Support

Configure multiple API keys for automatic rotation:

```json
{
  "providers": {
    "openai": {
      "base_url": "https://api.openai.com/v1",
      "api_keys_env": ["OPENAI_API_KEY", "OPENAI_API_KEY_2", "OPENAI_API_KEY_3"]
    }
  },
  "key_pools": {
    "openai": {
      "keys_env": ["OPENAI_API_KEY", "OPENAI_API_KEY_2"],
      "rotation_strategy": "round_robin"
    }
  }
}
```

Rotation strategies:
- `round_robin` (default): Cycle through keys sequentially
- `least_recently_used`: Use the key that hasn't been used longest
- `random`: Random selection

### Provider-Specific Options

```json
{
  "providers": {
    "example": {
      "base_url": "https://api.example.com/v1",
      "api_key_env": "EXAMPLE_API_KEY",
      "headers": {
        "X-Custom-Header": "value"
      },
      "param_mappings": {
        "max_completion_tokens": "max_tokens"
      },
      "special_handling": {
        "convert_content_list_to_string": true,
        "add_text_to_tool_calls": true
      },
      "rate_limit": {
        "requests_per_minute": 60,
        "retry_after_header": "retry-after"
      }
    }
  }
}
```

## Rate Limiting

LLMAO automatically handles rate limits:

1. **Detection**: Recognizes HTTP 429 and rate limit error messages
2. **Key Rotation**: When rate limited, automatically switches to another key
3. **Backoff**: Exponential backoff with jitter for retries
4. **Header Parsing**: Respects `retry-after` and `x-ratelimit-*` headers

```python
# With multiple keys configured, LLMAO will:
# 1. Use first available key
# 2. If rate limited, mark key and try next
# 3. Continue until success or all keys exhausted
response = client.completion(model="openai/gpt-4", messages=[...])
```

## Error Handling

```python
from llmao import LLMClient

client = LLMClient()

try:
    response = client.completion(
        model="openai/gpt-4",
        messages=[{"role": "user", "content": "Hello"}]
    )
except ValueError as e:
    # Invalid model format or provider not found
    print(f"Configuration error: {e}")
except RuntimeError as e:
    # Rate limited, auth failed, or no keys available
    print(f"Runtime error: {e}")
except ConnectionError as e:
    # Network or timeout error
    print(f"Connection error: {e}")
```

## Development

### Building from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install maturin
pip install maturin

# Development build
maturin develop

# Release build
maturin build --release
```

### Running Tests

```bash
# Rust tests
cargo test

# Python tests
pip install pytest pytest-asyncio
pytest tests/python/
```

## License

MIT License
