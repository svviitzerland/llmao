"""Example 1: Simple Single Model

Basic usage with environment variables for API keys.
Set CEREBRAS_API_KEY environment variable before running.
"""
from llmao_py import LLMClient

# Keys are loaded from environment variables automatically
# export CEREBRAS_API_KEY="your-key-here"

client = LLMClient()

print("Sending request...")
response = client.completion(
    model="cerebras/llama3.1-70b"
    [{"role": "user", "content": "Hello! Say hi in one word."}],
)

print(f"Response: {response['choices'][0]['message']['content']}")
