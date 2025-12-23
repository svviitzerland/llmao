//! Streaming Support
//!
//! Handles Server-Sent Events (SSE) streaming for chat completions.

use crate::api::completion::{Message, MessageContent, ToolCall, Usage};
use crate::error::{LlmaoError, Result};
use serde::{Deserialize, Serialize};

/// A streaming chunk from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Chunk ID
    pub id: String,

    /// Object type
    pub object: String,

    /// Creation timestamp
    pub created: u64,

    /// Model name
    pub model: String,

    /// Choices with deltas
    pub choices: Vec<StreamChoice>,

    /// Usage info (only in final chunk for some providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// A choice in a streaming chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    /// Choice index
    pub index: u32,

    /// The delta (partial message)
    pub delta: StreamDelta,

    /// Finish reason (set in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Delta content in a streaming chunk
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamDelta {
    /// Role (usually only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Content delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Tool calls delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// Delta for tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// Index in the tool_calls array
    pub index: u32,

    /// Tool call ID (only in first chunk for this tool call)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Type (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub call_type: Option<String>,

    /// Function delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionDelta>,
}

/// Delta for function calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDelta {
    /// Function name (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Arguments delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Accumulator for streaming chunks
#[derive(Debug, Default)]
pub struct StreamAccumulator {
    /// Accumulated content
    pub content: String,

    /// Accumulated tool calls
    pub tool_calls: Vec<ToolCallAccumulator>,

    /// Role from first chunk
    pub role: Option<String>,

    /// Finish reason from last chunk
    pub finish_reason: Option<String>,

    /// Response ID
    pub id: Option<String>,

    /// Model name
    pub model: Option<String>,

    /// Created timestamp
    pub created: Option<u64>,

    /// Usage from final chunk
    pub usage: Option<Usage>,
}

/// Accumulator for a single tool call
#[derive(Debug, Default, Clone)]
pub struct ToolCallAccumulator {
    pub id: String,
    pub call_type: String,
    pub name: String,
    pub arguments: String,
}

impl StreamAccumulator {
    /// Create a new accumulator
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a streaming chunk
    pub fn process_chunk(&mut self, chunk: &StreamChunk) -> Result<()> {
        // Store metadata from first chunk
        if self.id.is_none() {
            self.id = Some(chunk.id.clone());
            self.model = Some(chunk.model.clone());
            self.created = Some(chunk.created);
        }

        // Store usage if present (usually in final chunk)
        if chunk.usage.is_some() {
            self.usage = chunk.usage.clone();
        }

        // Process each choice
        for choice in &chunk.choices {
            // Store role from first chunk
            if let Some(role) = &choice.delta.role {
                if self.role.is_none() {
                    self.role = Some(role.clone());
                }
            }

            // Accumulate content
            if let Some(content) = &choice.delta.content {
                self.content.push_str(content);
            }

            // Accumulate tool calls
            if let Some(tool_calls) = &choice.delta.tool_calls {
                for tc_delta in tool_calls {
                    let idx = tc_delta.index as usize;

                    // Ensure we have enough slots
                    while self.tool_calls.len() <= idx {
                        self.tool_calls.push(ToolCallAccumulator::default());
                    }

                    let tc = &mut self.tool_calls[idx];

                    // Store ID and type from first chunk
                    if let Some(id) = &tc_delta.id {
                        tc.id = id.clone();
                    }
                    if let Some(call_type) = &tc_delta.call_type {
                        tc.call_type = call_type.clone();
                    }

                    // Accumulate function details
                    if let Some(func) = &tc_delta.function {
                        if let Some(name) = &func.name {
                            tc.name.push_str(name);
                        }
                        if let Some(args) = &func.arguments {
                            tc.arguments.push_str(args);
                        }
                    }
                }
            }

            // Store finish reason
            if let Some(reason) = &choice.finish_reason {
                self.finish_reason = Some(reason.clone());
            }
        }

        Ok(())
    }

    /// Convert to a final Message
    pub fn into_message(self) -> Message {
        let tool_calls = if self.tool_calls.is_empty() {
            None
        } else {
            Some(
                self.tool_calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        id: tc.id,
                        call_type: tc.call_type,
                        function: crate::api::completion::FunctionCall {
                            name: tc.name,
                            arguments: tc.arguments,
                        },
                    })
                    .collect(),
            )
        };

        Message {
            role: self.role.unwrap_or_else(|| "assistant".to_string()),
            content: MessageContent::Text(self.content),
            name: None,
            tool_calls,
            tool_call_id: None,
        }
    }
}

/// Parse SSE data line into a StreamChunk
pub fn parse_sse_line(line: &str) -> Result<Option<StreamChunk>> {
    // Skip empty lines and comments
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(None);
    }

    // Parse data: prefix
    if let Some(data) = line.strip_prefix("data: ") {
        let data = data.trim();

        // Check for [DONE] signal
        if data == "[DONE]" {
            return Ok(None);
        }

        // Parse JSON
        let chunk: StreamChunk = serde_json::from_str(data).map_err(|e| {
            LlmaoError::Stream(format!("Failed to parse SSE chunk: {}. Data: {}", e, data))
        })?;

        return Ok(Some(chunk));
    }

    // Ignore other event types (event:, id:, retry:)
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_line() {
        let line = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;

        let chunk = parse_sse_line(line).unwrap().unwrap();
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_sse_done() {
        let line = "data: [DONE]";
        assert!(parse_sse_line(line).unwrap().is_none());
    }

    #[test]
    fn test_stream_accumulator() {
        let mut acc = StreamAccumulator::new();

        // First chunk with role
        let chunk1 = StreamChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 12345,
            model: "gpt-4".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".to_string()),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage: None,
        };

        // Second chunk with more content
        let chunk2 = StreamChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 12345,
            model: "gpt-4".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: None,
                    content: Some(" World".to_string()),
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: None,
        };

        acc.process_chunk(&chunk1).unwrap();
        acc.process_chunk(&chunk2).unwrap();

        assert_eq!(acc.content, "Hello World");
        assert_eq!(acc.role, Some("assistant".to_string()));
        assert_eq!(acc.finish_reason, Some("stop".to_string()));

        let message = acc.into_message();
        assert_eq!(message.role, "assistant");
        assert_eq!(message.content.to_string_content(), "Hello World");
    }
}
