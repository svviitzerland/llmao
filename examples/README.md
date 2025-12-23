# LLMAO Examples

Examples demonstrating different ways to configure and use LLMAO.

## Examples

| File | Description |
|------|-------------|
| [`simple_chat.py`](simple_chat.py) | Basic usage with environment variables |
| [`config_file.py`](config_file.py) | Load configuration from `config.json` |
| [`multi_provider.py`](multi_provider.py) | Multiple providers with key rotation |
| [`programmatic_config.py`](programmatic_config.py) | Dictionary-based configuration (no config file) |
| [`custom_provider.py`](custom_provider.py) | Custom provider with `base_url` |

## Quick Start

```bash
# Install
pip install llmao-py

# Set your API key
export CEREBRAS_API_KEY="your-key-here"

# Run example
python simple_chat.py
```
