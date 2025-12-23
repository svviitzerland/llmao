"""
LLMAO - Lightweight LLM API Orchestrator

A fast, multi-provider LLM client with rate limiting and key rotation.
"""

from ._llmao import LLMClient, completion, __version__

__all__ = ["LLMClient", "completion", "__version__"]
