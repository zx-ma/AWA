use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AwaError, AwaResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub models: ModelPaths,
    pub camera: CameraConfig,
    pub auth: AuthConfig,
    pub store: StoreConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPaths {
    pub scrfd: PathBuf,
    pub arcface: PathBuf,
    pub minifas: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub rgb_device: PathBuf,
    pub ir_device: Option<PathBuf>,
    pub rgb_width: u32,
    pub rgb_height: u32,
    pub ir_width: u32,
    pub ir_height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMode {
    RgbOnly,
    IrPreferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub threshold: f32,
    pub liveness_threshold: f32,
    pub ir_min_brightness: f32,
    pub mode: AuthMode,
    pub max_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub base_dir: PathBuf,
}

impl Config {
    pub fn load(path: &Path) -> AwaResult<Self> {
        let data = std::fs::read_to_string(path)?;
        toml::from_str(&data)
            .map_err(|e| AwaError::Config(format!("parse {}: {}", path.display(), e)))
    }

    pub fn default_search_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join(".config/awa/config.toml"));
        }
        paths.push(PathBuf::from("/etc/awa/config.toml"));
        paths
    }

    pub fn discover() -> AwaResult<(PathBuf, Self)> {
        for p in Self::default_search_paths() {
            if p.exists() {
                let cfg = Self::load(&p)?;
                return Ok((p, cfg));
            }
        }
        Err(AwaError::Config(
            "no config file found in $HOME/.config/awa/config.toml or /etc/awa/config.toml".into(),
        ))
    }
}
