"""Example 4: Custom Provider

Using a provider that is NOT in the built-in registry.
Specify base_url for custom API endpoints.
"""
from llmao_py import LLMClient

# Define custom provider config
config = {
    "my_custom_llm/model-v1": {
        "base_url": "https://api.custom-provider.com/v1",
        "keys": ["your-custom-api-key"],
        "headers": {
            "X-Custom-Header": "custom-value"
        },
        "param_mappings": {
            "max_completion_tokens": "max_tokens"
        }
    }
}

client = LLMClient(config=config)

print("Sending request to custom provider...")
try:
    response = client.completion([{"role": "user", "content": "Hello!"}])
    print(f"Response: {response['choices'][0]['message']['content']}")
except Exception as e:
    print(f"Error: {e}")
