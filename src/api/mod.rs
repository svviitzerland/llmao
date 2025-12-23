//! API Module
//!
//! Chat completion API types and streaming support.

pub mod completion;
pub mod streaming;

pub use completion::{
    Choice, CompletionRequest, CompletionResponse, ContentPart, FunctionCall, FunctionDefinition,
    ImageUrl, Message, MessageContent, Tool, ToolCall, ToolChoice, Usage,
};
pub use streaming::{parse_sse_line, StreamAccumulator, StreamChoice, StreamChunk, StreamDelta};
