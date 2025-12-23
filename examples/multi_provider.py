"""Example 2: Multiple Providers

Configure multiple providers with different API keys.
Specify model when calling completion() to choose which provider to use.
"""
from llmao_py import LLMClient

# Config with multiple providers
config = {
    "cerebras": {
        "models": ["llama3.1-8b"],
        "keys": ["your-cerebras-key"],
        "rotation_strategy": "round_robin"
    },
    "groq": {
        "models": ["llama-3.1-8b"],
        "keys": ["your-groq-key"]
    }
}

print("Initializing with multiple providers...")
client = LLMClient(config=config)
print(f"Configured models: {client.models()}")

# When multiple models are configured, specify which one to use
print("\n--- Testing Cerebras ---")
try:
    response = client.completion(
        [{"role": "user", "content": "Hello!"}],
        model="cerebras/llama3.1-8b"
    )
    print(f"Response: {response['choices'][0]['message']['content'][:50]}")
except Exception as e:
    print(f"Error: {e}")

print("\n--- Testing Groq ---")
try:
    response = client.completion(
        [{"role": "user", "content": "Hello!"}],
        model="groq/llama-3.1-8b"
    )
    print(f"Response: {response['choices'][0]['message']['content'][:50]}")
except Exception as e:
    print(f"Error: {e}")
