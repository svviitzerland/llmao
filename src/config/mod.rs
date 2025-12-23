//! Configuration Module
//!
//! Handles provider configuration loading and validation.

pub mod loader;
pub mod provider;

pub use loader::ConfigLoader;
pub use provider::{
    KeyPoolConfig, ProviderConfig, ProvidersConfig, RateLimitConfig, RotationStrategy,
    SpecialHandling,
};
