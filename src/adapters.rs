use std::path::PathBuf;

use crate::config::{AdapterConfig, AppConfig, SourceMissingBehavior};

#[derive(Debug, Clone)]
pub struct Adapter {
    pub name: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub on_source_missing: SourceMissingBehavior,
}

impl Adapter {
    pub fn from_config(name: &str, config: &AdapterConfig) -> Option<Self> {
        config.enabled.then(|| Self {
            name: name.to_string(),
            source: config.source.clone(),
            target: config.target.clone(),
            on_source_missing: config.on_source_missing,
        })
    }
}

pub fn enabled_adapters(config: &AppConfig) -> Vec<Adapter> {
    let mut adapters = Vec::new();
    if let Some(adapter) = Adapter::from_config("claude", &config.adapters.claude) {
        adapters.push(adapter);
    }
    adapters
}
