"""
Basic usage example for LLMAO.

Set your API key in environment:
    export GROQ_API_KEY="your-key"

Then run:
    python basic_usage.py
"""

from llmao import LLMClient

def main():
    # Create client (automatically loads .env if present)
    client = LLMClient()

    # List available providers
    print("Available providers:")
    for provider in client.providers():
        info = client.provider_info(provider)
        has_key = "configured" if info and info["has_keys"] else "no key"
        print(f"  - {provider} ({has_key})")

    # Make a completion request
    # Format: provider/model
    response = client.completion(
        model="groq/llama-3.1-70b-versatile",
        messages=[
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is the capital of Indonesia?"}
        ],
        temperature=0.7,
        max_tokens=100
    )

    # Print the response
    print("\nResponse:")
    print(response["choices"][0]["message"]["content"])

    # Print usage stats
    if "usage" in response:
        usage = response["usage"]
        print(f"\nTokens: {usage['prompt_tokens']} prompt, {usage['completion_tokens']} completion")

if __name__ == "__main__":
    main()
