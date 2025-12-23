"""
Custom provider configuration example.

This example shows how to use a custom providers.json file
to add your own providers or override default settings.
"""

import json
import tempfile
import os
from llmao import LLMClient

# Custom provider configuration
CUSTOM_CONFIG = {
    "providers": {
        # Override a built-in provider
        "groq": {
            "base_url": "https://api.groq.com/openai/v1",
            "api_key_env": "GROQ_API_KEY",
            # Custom rate limit settings
            "rate_limit": {
                "requests_per_minute": 30
            }
        },
        # Add a completely custom provider
        "my_custom_provider": {
            "base_url": "https://api.my-provider.com/v1",
            "api_key_env": "MY_CUSTOM_API_KEY",
            "param_mappings": {
                "max_completion_tokens": "max_tokens"
            },
            "headers": {
                "X-Custom-Header": "my-value"
            }
        }
    },
    "key_pools": {
        # Use multiple keys with rotation
        "groq": {
            "keys_env": ["GROQ_API_KEY", "GROQ_API_KEY_2"],
            "rotation_strategy": "round_robin"
        }
    }
}

def main():
    # Write custom config to a temp file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        json.dump(CUSTOM_CONFIG, f)
        config_path = f.name
    
    try:
        # Create client with custom config
        client = LLMClient(config_path=config_path)
        
        print("Providers after custom config:")
        for provider in client.providers():
            info = client.provider_info(provider)
            if info:
                print(f"  - {provider}: {info['base_url']}")
        
        # The custom provider is now available
        # (would fail without proper API key)
        # client.completion(
        #     model="my_custom_provider/some-model",
        #     messages=[{"role": "user", "content": "Hello"}]
        # )
        
    finally:
        os.unlink(config_path)

if __name__ == "__main__":
    main()
