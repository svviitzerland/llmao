<div align="center">

<img src="assets/logo.svg" alt="LLMAO Logo" width="150" height="150" />

# LLMAO

**Lightweight LLM API Orchestrator**

*One interface for all your LLM APIs, fast and simple*

[![Python](https://img.shields.io/badge/python-3.9+-blue?style=flat&logo=python&logoColor=white)](https://pypi.org/project/llmao/)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-dea584?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/github/license/svviitzerland/llmao?style=flat)](LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/svviitzerland/llmao/ci.yml?style=flat&logo=github&label=CI)](https://github.com/svviitzerland/llmao/actions)

</div>

---

A unified Python interface for multiple LLM providers with automatic key rotation and rate limit handling. Built with Rust core for performance.

## Installation

```bash
pip install llmao-py
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
```

## Supported Providers

<details>
<summary>View all providers</summary>

| Provider | Model Usage | Environment Variable |
|----------|-------------|---------------------|
| OpenAI | `openai/your-model-name` | `OPENAI_API_KEY` |
| Anthropic | `anthropic/your-model-name` | `ANTHROPIC_API_KEY` |
| Groq | `groq/your-model-name` | `GROQ_API_KEY` |
| Cerebras | `cerebras/your-model-name` | `CEREBRAS_API_KEY` |
| Together | `together/your-model-name` | `TOGETHER_API_KEY` |
| OpenRouter | `openrouter/your-model-name` | `OPENROUTER_API_KEY` |
| DeepSeek | `deepseek/your-model-name` | `DEEPSEEK_API_KEY` |
| Mistral | `mistral/your-model-name` | `MISTRAL_API_KEY` |
| Fireworks | `fireworks/your-model-name` | `FIREWORKS_API_KEY` |
| Perplexity | `perplexity/your-model-name` | `PERPLEXITY_API_KEY` |
| SambaNova | `sambanova/your-model-name` | `SAMBANOVA_API_KEY` |
| NVIDIA | `nvidia/your-model-name` | `NVIDIA_API_KEY` |
| Hyperbolic | `hyperbolic/your-model-name` | `HYPERBOLIC_API_KEY` |
| DeepInfra | `deepinfra/your-model-name` | `DEEPINFRA_API_KEY` |
| Novita | `novita/your-model-name` | `NOVITA_API_KEY` |
| Xiaomi MiMo | `xiaomi_mimo/your-model-name` | `XIAOMI_MIMO_API_KEY` |
| Venice AI | `veniceai/your-model-name` | `VENICE_AI_API_KEY` |
| GLHF | `glhf/your-model-name` | `GLHF_API_KEY` |
| Lepton | `lepton/your-model-name` | `LEPTON_API_KEY` |
| Anyscale | `anyscale/your-model-name` | `ANYSCALE_API_KEY` |
| Ollama | `ollama/your-model-name` | `OLLAMA_API_KEY` |
| LM Studio | `lmstudio/your-model-name` | `LMSTUDIO_API_KEY` |
| PublicAI | `publicai/your-model-name` | `PUBLICAI_API_KEY` |
| Helicone | `helicone/your-model-name` | `HELICONE_API_KEY` |

</details>

## Key Rotation

Automatic failover when rate limited. Configure multiple keys in your `config.json`:

```json
{
  "openai/gpt-4": {
    "keys": ["sk-key1", "sk-key2", "sk-key3"],
    "rotation_strategy": "round_robin"
  }
}
```

Or use environment variables (suffix with `_2`, `_3`, etc.):
```bash
export OPENAI_API_KEY="sk-key1"
export OPENAI_API_KEY_2="sk-key2"
export OPENAI_API_KEY_3="sk-key3"
```

## Configuration

LLMAO supports multiple configuration methods. See the [`examples/`](examples/) directory for complete, runnable code for each scenario.

## Contributing Providers

Want to add a new provider to LLMAO's built-in registry? 

The `registry.json` file contains provider metadata (base URLs, default headers, etc.). Fork the repository, add your provider, and submit a pull request:

```json
{
  "your_provider": {
    "base_url": "https://api.yourprovider.com/v1",
    "api_key_env": "YOUR_PROVIDER_API_KEY"
  }
}
```

Once merged, all users can use your provider without specifying `base_url`!

## API Reference

```python
from llmao import LLMClient, completion

# Client-based
client = LLMClient(config_path="./config.json")
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

