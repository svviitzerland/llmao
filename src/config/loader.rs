//! Configuration Loader
//!
//! Handles loading and merging provider configurations from multiple sources.

use crate::config::provider::ProvidersConfig;
use crate::error::{LlmaoError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration loader with support for multiple sources
pub struct ConfigLoader {
    config: ProvidersConfig,
}

impl ConfigLoader {
    /// Create a new config loader and load from default locations
    pub fn new() -> Result<Self> {
        let mut loader = Self {
            config: ProvidersConfig {
                providers: HashMap::new(),
                key_pools: HashMap::new(),
            },
        };

        // Load built-in defaults first
        loader.load_builtin_defaults()?;

        // Then load from file system (can override built-ins)
        loader.load_from_default_paths()?;

        Ok(loader)
    }

    /// Create a loader with a specific config file
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let mut loader = Self {
            config: ProvidersConfig {
                providers: HashMap::new(),
                key_pools: HashMap::new(),
            },
        };

        loader.load_builtin_defaults()?;
        loader.load_from_file(path)?;

        Ok(loader)
    }

    /// Load built-in provider defaults
    fn load_builtin_defaults(&mut self) -> Result<()> {
        let defaults = include_str!("../../providers.json");
        let config: ProvidersConfig = serde_json::from_str(defaults).map_err(|e| {
            LlmaoError::Config(format!("Failed to parse built-in providers.json: {}", e))
        })?;

        self.merge_config(config);
        Ok(())
    }

    /// Load configuration from default paths
    fn load_from_default_paths(&mut self) -> Result<()> {
        let paths = Self::get_config_paths();

        for path in paths {
            if path.exists() {
                self.load_from_file(&path)?;
            }
        }

        Ok(())
    }

    /// Get list of config paths to check
    fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Environment variable
        if let Ok(custom_path) = std::env::var("LLMAO_PROVIDERS_PATH") {
            paths.push(PathBuf::from(custom_path));
        }

        // 2. Current directory
        paths.push(PathBuf::from("providers.json"));
        paths.push(PathBuf::from("llmao.json"));

        // 3. User config directory
        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("llmao").join("providers.json"));
        }

        // 4. Home directory
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join(".llmao").join("providers.json"));
        }

        paths
    }

    /// Load configuration from a specific file
    fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            LlmaoError::Config(format!("Failed to read {}: {}", path.display(), e))
        })?;

        let config: ProvidersConfig = serde_json::from_str(&content).map_err(|e| {
            LlmaoError::Config(format!("Failed to parse {}: {}", path.display(), e))
        })?;

        self.merge_config(config);
        Ok(())
    }

    /// Merge another config into this one (later configs override earlier)
    fn merge_config(&mut self, other: ProvidersConfig) {
        for (name, provider) in other.providers {
            self.config.providers.insert(name, provider);
        }

        for (name, pool) in other.key_pools {
            self.config.key_pools.insert(name, pool);
        }
    }

    /// Get the loaded configuration
    pub fn config(&self) -> &ProvidersConfig {
        &self.config
    }

    /// Take ownership of the configuration
    pub fn into_config(self) -> ProvidersConfig {
        self.config
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config: ProvidersConfig {
                providers: HashMap::new(),
                key_pools: HashMap::new(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_builtin_defaults() {
        let loader = ConfigLoader::new().unwrap();
        assert!(!loader.config().providers.is_empty());
    }

    #[test]
    fn test_load_from_custom_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{
                "providers": {{
                    "custom_provider": {{
                        "base_url": "https://custom.api.com/v1",
                        "api_key_env": "CUSTOM_API_KEY"
                    }}
                }},
                "key_pools": {{}}
            }}"#
        )
        .unwrap();

        let loader = ConfigLoader::from_path(file.path()).unwrap();
        assert!(loader.config().providers.contains_key("custom_provider"));
    }

    #[test]
    fn test_merge_configs() {
        let mut loader = ConfigLoader::new().unwrap();
        let initial_count = loader.config().providers.len();

        // Create a custom config that adds a new provider
        let custom = ProvidersConfig {
            providers: [(
                "new_provider".to_string(),
                crate::config::provider::ProviderConfig {
                    base_url: "https://new.api.com".to_string(),
                    api_key_env: Some("NEW_KEY".to_string()),
                    api_keys_env: None,
                    api_base_env: None,
                    models: vec![],
                    param_mappings: HashMap::new(),
                    headers: HashMap::new(),
                    rate_limit: None,
                    special_handling: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            key_pools: HashMap::new(),
        };

        loader.merge_config(custom);
        assert_eq!(loader.config().providers.len(), initial_count + 1);
    }
}
