"""
Multi-provider example demonstrating fallback between providers.

This example shows how to try multiple providers and fall back
to the next one if the current one fails (e.g., rate limited).
"""

from llmao import LLMClient

# List of models to try in order (provider/model format)
MODELS = [
    "groq/llama-3.1-70b-versatile",
    "cerebras/llama3.1-70b",
    "together/meta-llama/Llama-3.3-70B-Instruct-Turbo",
]

def completion_with_fallback(client, messages, **kwargs):
    """Try multiple providers until one succeeds."""
    last_error = None
    
    for model in MODELS:
        try:
            print(f"Trying {model}...")
            response = client.completion(
                model=model,
                messages=messages,
                **kwargs
            )
            print(f"Success with {model}")
            return response
        except RuntimeError as e:
            print(f"Failed: {e}")
            last_error = e
            continue
        except ValueError as e:
            # Provider not found or invalid model
            print(f"Skipping: {e}")
            continue
    
    raise last_error or RuntimeError("All providers failed")

def main():
    client = LLMClient()
    
    messages = [
        {"role": "user", "content": "Say hello in 3 different languages."}
    ]
    
    response = completion_with_fallback(
        client,
        messages,
        temperature=0.7,
        max_tokens=200
    )
    
    print("\nResponse:")
    print(response["choices"][0]["message"]["content"])

if __name__ == "__main__":
    main()
