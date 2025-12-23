"""Example 5: Config File

Load configuration from a JSON file instead of defining in code.
"""
from llmao_py import LLMClient

# Load config from file
# Create a config.json with your settings first (see example below)
client = LLMClient(config_path="config.json")

print(f"Configured models: {client.models()}")

print("\nSending request...")
response = client.completion([{"role": "user", "content": "Hello!"}])
print(f"Response: {response['choices'][0]['message']['content']}")


# Example config.json:
# {
#     "cerebras": {
#         "models": ["llama3.1-8b"],
#         "keys": ["your-api-key"],
#         "rotation_strategy": "round_robin"
#     }
# }
