//! Model Routing
//!
//! Handles parsing and routing of model identifiers.

use crate::error::{LlmaoError, Result};

/// Parsed model identifier
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRoute {
    /// Provider name (e.g., "openai", "groq")
    pub provider: String,

    /// Model name (e.g., "gpt-4", "llama-3.1-70b")
    pub model: String,

    /// Optional variant/deployment (e.g., for Azure deployments)
    pub variant: Option<String>,
}

impl ModelRoute {
    /// Parse a model string in the format "provider/model" or "provider/model/variant"
    pub fn parse(model_string: &str) -> Result<Self> {
        let parts: Vec<&str> = model_string.split('/').collect();

        match parts.len() {
            2 => Ok(Self {
                provider: parts[0].to_string(),
                model: parts[1].to_string(),
                variant: None,
            }),
            3 => Ok(Self {
                provider: parts[0].to_string(),
                model: parts[1].to_string(),
                variant: Some(parts[2].to_string()),
            }),
            _ => Err(LlmaoError::Config(format!(
                "Invalid model format '{}'. Expected 'provider/model' or 'provider/model/variant'",
                model_string
            ))),
        }
    }

    /// Get the full model identifier (for API calls)
    pub fn model_id(&self) -> String {
        if let Some(ref variant) = self.variant {
            format!("{}/{}", self.model, variant)
        } else {
            self.model.clone()
        }
    }
}

impl std::fmt::Display for ModelRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref variant) = self.variant {
            write!(f, "{}/{}/{}", self.provider, self.model, variant)
        } else {
            write!(f, "{}/{}", self.provider, self.model)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let route = ModelRoute::parse("openai/gpt-4").unwrap();
        assert_eq!(route.provider, "openai");
        assert_eq!(route.model, "gpt-4");
        assert_eq!(route.variant, None);
    }

    #[test]
    fn test_parse_with_variant() {
        let route = ModelRoute::parse("azure/gpt-4/my-deployment").unwrap();
        assert_eq!(route.provider, "azure");
        assert_eq!(route.model, "gpt-4");
        assert_eq!(route.variant, Some("my-deployment".to_string()));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(ModelRoute::parse("just-a-model").is_err());
        assert!(ModelRoute::parse("a/b/c/d").is_err());
    }

    #[test]
    fn test_model_id() {
        let simple = ModelRoute::parse("openai/gpt-4").unwrap();
        assert_eq!(simple.model_id(), "gpt-4");

        let with_variant = ModelRoute::parse("azure/gpt-4/deployment").unwrap();
        assert_eq!(with_variant.model_id(), "gpt-4/deployment");
    }

    #[test]
    fn test_display() {
        let simple = ModelRoute::parse("openai/gpt-4").unwrap();
        assert_eq!(format!("{}", simple), "openai/gpt-4");

        let with_variant = ModelRoute::parse("azure/gpt-4/deployment").unwrap();
        assert_eq!(format!("{}", with_variant), "azure/gpt-4/deployment");
    }
}
