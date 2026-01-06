//! Configuration file handling for topo
//!
//! Supports topo.config.json

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(String),

    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<DevConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<PathsConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<PluginConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_mode")]
    pub mode: BuildMode,

    #[serde(default = "default_output")]
    pub output: String,

    #[serde(default = "default_true")]
    pub minify: bool,

    #[serde(default = "default_true")]
    pub sourcemap: bool,

    #[serde(default = "default_target")]
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildMode {
    Spa,
    Ssg,
    Ssr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevConfig {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_true")]
    pub open: bool,

    #[serde(default = "default_true")]
    pub hmr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    #[serde(default = "default_style_framework")]
    pub framework: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tailwind: Option<TailwindConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailwindConfig {
    /// Enable/disable Tailwind CSS
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Tailwind version (e.g., "3.4.0")
    #[serde(default = "default_tailwind_version")]
    pub version: String,

    /// Use CDN or local build
    #[serde(default = "default_tailwind_cdn")]
    pub cdn: bool,

    /// Custom CDN URL (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdn_url: Option<String>,

    /// Path to tailwind.config.js
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_pages_path")]
    pub pages: String,

    #[serde(default = "default_components_path")]
    pub components: String,

    #[serde(default = "default_stores_path")]
    pub stores: String,

    #[serde(default = "default_services_path")]
    pub services: String,

    #[serde(default = "default_layouts_path")]
    pub layouts: String,

    #[serde(default = "default_public_path")]
    pub public: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<ApiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_api_base_url")]
    pub base_url: String,

    #[serde(default = "default_api_timeout")]
    pub timeout: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginConfig {
    Name(String),
    WithOptions { name: String, options: serde_json::Value },
}

// Default value functions
fn default_mode() -> BuildMode { BuildMode::Spa }
fn default_output() -> String { "dist".to_string() }
fn default_true() -> bool { true }
fn default_target() -> String { "es2022".to_string() }
fn default_port() -> u16 { 7090 }
fn default_host() -> String { "localhost".to_string() }
fn default_style_framework() -> String { "tailwind".to_string() }
fn default_tailwind_version() -> String { "3.4.0".to_string() }
fn default_tailwind_cdn() -> bool { true }
fn default_pages_path() -> String { "src/pages".to_string() }
fn default_components_path() -> String { "src/components".to_string() }
fn default_stores_path() -> String { "src/stores".to_string() }
fn default_services_path() -> String { "src/services".to_string() }
fn default_layouts_path() -> String { "src/layouts".to_string() }
fn default_public_path() -> String { "public".to_string() }
fn default_api_base_url() -> String { "/api".to_string() }
fn default_api_timeout() -> u32 { 5000 }

impl Config {
    /// Load configuration from file
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from current directory
    pub fn load_from_cwd() -> Result<Self, ConfigError> {
        let path = Path::new("topo.config.json");
        Self::load(path)
    }

    /// Try to load configuration, return default if not found
    pub fn load_or_default() -> Self {
        Self::load_from_cwd().unwrap_or_default()
    }

    /// Save configuration to file
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get build configuration with defaults
    pub fn build_config(&self) -> BuildConfig {
        self.build.clone().unwrap_or_default()
    }

    /// Get dev configuration with defaults
    pub fn dev_config(&self) -> DevConfig {
        self.dev.clone().unwrap_or_default()
    }

    /// Get paths configuration with defaults
    pub fn paths_config(&self) -> PathsConfig {
        self.paths.clone().unwrap_or_default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project: Some(ProjectConfig {
                name: "my-app".to_string(),
                version: "0.1.0".to_string(),
                description: None,
            }),
            build: Some(BuildConfig::default()),
            dev: Some(DevConfig::default()),
            style: Some(StyleConfig::default()),
            paths: Some(PathsConfig::default()),
            runtime: None,
            plugins: None,
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            mode: BuildMode::Spa,
            output: "dist".to_string(),
            minify: true,
            sourcemap: true,
            target: "es2022".to_string(),
        }
    }
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            port: 7090,
            host: "localhost".to_string(),
            open: true,
            hmr: true,
        }
    }
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            framework: "tailwind".to_string(),
            config: None,
            tailwind: Some(TailwindConfig::default()),
        }
    }
}

impl Default for TailwindConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            version: "3.4.0".to_string(),
            cdn: true,
            cdn_url: None,
            config_path: None,
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            pages: "src/pages".to_string(),
            components: "src/components".to_string(),
            stores: "src/stores".to_string(),
            services: "src/services".to_string(),
            layouts: "src/layouts".to_string(),
            public: "public".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let json = r#"{
            "project": {
                "name": "test-app",
                "version": "1.0.0"
            },
            "build": {
                "mode": "spa",
                "output": "dist"
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.project.as_ref().unwrap().name, "test-app");
        assert_eq!(config.build_config().mode, BuildMode::Spa);
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.build_config().mode, BuildMode::Spa);
        assert_eq!(config.dev_config().port, 7090);
    }
}
