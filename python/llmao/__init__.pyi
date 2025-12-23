"""
Type stubs for LLMAO - Lightweight LLM API Orchestrator
"""

from typing import Any, Iterator, Optional, TypedDict

class Message(TypedDict, total=False):
    role: str
    content: str
    name: Optional[str]
    tool_calls: Optional[list[dict[str, Any]]]
    tool_call_id: Optional[str]

class Choice(TypedDict, total=False):
    index: int
    message: Message
    finish_reason: Optional[str]

class Usage(TypedDict):
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int

class CompletionResponse(TypedDict, total=False):
    id: str
    object: str
    created: int
    model: str
    choices: list[Choice]
    usage: Optional[Usage]

class ProviderInfo(TypedDict):
    name: str
    base_url: str
    models: list[str]
    has_keys: bool

class LLMClient:
    """
    Lightweight LLM API client with multi-provider support.
    
    Args:
        config_path: Optional path to a custom providers.json file.
    
    Example:
        ```python
        from llmao import LLMClient
        
        client = LLMClient()
        response = client.completion(
            model="openai/gpt-4",
            messages=[{"role": "user", "content": "Hello!"}]
        )
        print(response["choices"][0]["message"]["content"])
        ```
    """
    
    def __init__(self, config_path: Optional[str] = None) -> None: ...
    
    def completion(
        self,
        model: str,
        messages: list[dict[str, Any]],
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
        stream: Optional[bool] = None,
        **kwargs: Any
    ) -> CompletionResponse:
        """
        Create a chat completion.
        
        Args:
            model: Model identifier in format "provider/model" or "provider/model/variant".
                   Examples: "openai/gpt-4", "groq/llama-3.1-70b-versatile"
            messages: List of message dicts with 'role' and 'content' keys.
            temperature: Sampling temperature (0.0 to 2.0).
            max_tokens: Maximum tokens to generate.
            stream: Enable streaming (not yet implemented).
            **kwargs: Additional provider-specific parameters.
        
        Returns:
            CompletionResponse dict with id, choices, usage, etc.
        
        Raises:
            ValueError: If model format is invalid or provider not found.
            RuntimeError: If rate limited or no API keys available.
            ConnectionError: If request fails.
        """
        ...
    
    def providers(self) -> list[str]:
        """
        List available provider names.
        
        Returns:
            List of provider names (e.g., ["openai", "groq", "anthropic"])
        """
        ...
    
    def provider_info(self, name: str) -> Optional[ProviderInfo]:
        """
        Get information about a specific provider.
        
        Args:
            name: Provider name (e.g., "openai")
        
        Returns:
            ProviderInfo dict or None if provider not found.
        """
        ...

def completion(
    model: str,
    messages: list[dict[str, Any]],
    temperature: Optional[float] = None,
    max_tokens: Optional[int] = None,
    **kwargs: Any
) -> CompletionResponse:
    """
    Quick completion without explicit client initialization.
    
    Args:
        model: Model identifier in format "provider/model".
        messages: List of message dicts.
        temperature: Sampling temperature.
        max_tokens: Maximum tokens to generate.
        **kwargs: Additional parameters.
    
    Returns:
        CompletionResponse dict.
    
    Example:
        ```python
        from llmao import completion
        
        response = completion(
            model="groq/llama-3.1-70b-versatile",
            messages=[{"role": "user", "content": "Hello!"}]
        )
        ```
    """
    ...

__version__: str
