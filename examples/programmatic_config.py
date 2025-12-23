"""Example 3: Programmatic Configuration

Configure LLMAO directly with a dictionary (no config file needed).
When models are defined in config, you don't need to specify model in completion().
"""
from llmao_py import LLMClient

# Define configuration in code
config = {
    "cerebras": {
        "models": ["llama3.1-8b", "zai-glm-4.6"],
        "keys": [
            "your-cerebras-key-1",
            "your-cerebras-key-2",
        ],
        "rotation_strategy": "round_robin"
    }
}

print("Initializing with dictionary config...")
client = LLMClient(config=config)

print(f"Configured models: {client.models()}")

# No need to specify model - uses the first one from config
print("\nSending request (model auto-selected from config)...")
response = client.completion([{"role": "user", "content": "Hello!"}])
print(f"Response: {response['choices'][0]['message']['content']}")
