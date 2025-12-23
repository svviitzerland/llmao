<div align="center">

<img src="assets/logo.svg" alt="LLMAO Logo" width="150" height="150" />

# LLMAO

**Lightweight LLM API Orchestrator**

*Like litellm, but written in Rust. Because life's too short for slow API calls.*

[![PyPI](https://img.shields.io/pypi/v/llmao?style=flat&logo=pypi&logoColor=white&label=PyPI)](https://pypi.org/project/llmao/)
[![Python](https://img.shields.io/pypi/pyversions/llmao?style=flat&logo=python&logoColor=white)](https://pypi.org/project/llmao/)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-dea584?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/github/license/svviitzerland/llmao?style=flat)](LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/svviitzerland/llmao/ci.yml?style=flat&logo=github&label=CI)](https://github.com/svviitzerland/llmao/actions)

---

**20+ Providers** | **Auto Key Rotation** | **Rate Limit Handling** | **Blazingly Fast**

</div>

---

## Why LLMAO?

| Feature | LLMAO | litellm |
|---------|-------|---------|
| Core Language | Rust | Python |
| Startup Time | ~5ms | ~500ms |
| Memory Usage | Low | Higher |
| Key Rotation | Built-in | Manual |
| Rate Limiting | Automatic | Limited |

## Installation

```bash
pip install llmao
```

## Quick Start

```python
from llmao import LLMClient

client = LLMClient()

response = client.completion(
    model="groq/llama-3.1-70b-versatile",
    messages=[{"role": "user", "content": "Hello!"}]
)

print(response["choices"][0]["message"]["content"])
```

## Model Format

Use `provider/model` routing:

```python
# OpenAI
client.completion(model="openai/gpt-4o", messages=[...])

# Anthropic
client.completion(model="anthropic/claude-3-5-sonnet-20241022", messages=[...])

# Groq
client.completion(model="groq/llama-3.3-70b-versatile", messages=[...])

# Cerebras
client.completion(model="cerebras/llama3.1-70b", messages=[...])

# And 20+ more providers...
```

## Supported Providers

<details>
<summary>View all providers</summary>

| Provider | Environment Variable |
|----------|---------------------|
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| Groq | `GROQ_API_KEY` |
| Cerebras | `CEREBRAS_API_KEY` |
| Together | `TOGETHER_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| Mistral | `MISTRAL_API_KEY` |
| Fireworks | `FIREWORKS_API_KEY` |
| Perplexity | `PERPLEXITY_API_KEY` |
| SambaNova | `SAMBANOVA_API_KEY` |
| NVIDIA | `NVIDIA_API_KEY` |
| Hyperbolic | `HYPERBOLIC_API_KEY` |
| DeepInfra | `DEEPINFRA_API_KEY` |
| Novita | `NOVITA_API_KEY` |
| Xiaomi MiMo | `XIAOMI_MIMO_API_KEY` |
| Venice AI | `VENICE_AI_API_KEY` |
| GLHF | `GLHF_API_KEY` |
| Lepton | `LEPTON_API_KEY` |
| Anyscale | `ANYSCALE_API_KEY` |
| Ollama | `OLLAMA_API_KEY` |
| LM Studio | `LMSTUDIO_API_KEY` |

</details>

## Key Rotation

Automatic failover when rate limited:

```python
# Set multiple keys
export OPENAI_API_KEY="sk-key1"
export OPENAI_API_KEY_2="sk-key2"
export OPENAI_API_KEY_3="sk-key3"
```

```json
{
  "key_pools": {
    "openai": {
      "keys_env": ["OPENAI_API_KEY", "OPENAI_API_KEY_2", "OPENAI_API_KEY_3"],
      "rotation_strategy": "round_robin"
    }
  }
}
```

Strategies: `round_robin`, `least_recently_used`, `random`

## Custom Providers

Create `providers.json` in your project:

```json
{
  "providers": {
    "my_provider": {
      "base_url": "https://api.my-provider.com/v1",
      "api_key_env": "MY_PROVIDER_API_KEY"
    }
  }
}
```

## API Reference

```python
from llmao import LLMClient, completion

# Client-based
client = LLMClient(config_path="./providers.json")
client.completion(model, messages, temperature=0.7, max_tokens=100)
client.providers()  # List available providers
client.provider_info("openai")  # Get provider details

# Quick function
completion(model, messages, **kwargs)
```

## Development

```bash
# Build from source
pip install maturin
maturin develop

# Run tests
cargo test
```

## License

MIT

