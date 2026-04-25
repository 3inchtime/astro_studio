use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "astro_studio.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub log: LogConfig,
    pub api: ApiConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
    pub save_to_file: bool,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    pub timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub thumbnail_size: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            log: LogConfig::default(),
            api: ApiConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            save_to_file: false,
            file_path: "astro_studio.log".to_string(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 120,
            max_retries: 0,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            thumbnail_size: 256,
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .expect("Cannot determine config directory")
            .join("astro-studio")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join(CONFIG_FILE)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => {
                        log::info!("Loaded config from {}", path.display());
                        return config;
                    }
                    Err(e) => log::warn!("Failed to parse config: {}, using defaults", e),
                },
                Err(e) => log::warn!("Failed to read config: {}, using defaults", e),
            }
        }
        let config = Self::default();
        if let Err(e) = config.save() {
            log::warn!("Failed to save default config: {}", e);
        }
        config
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("Create config dir failed: {}", e))?;

        let path = Self::config_path();
        let content = toml::to_string_pretty(self).map_err(|e| format!("Serialize config failed: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("Write config failed: {}", e))?;

        log::info!("Config saved to {}", path.display());
        Ok(())
    }
}

pub fn init_logger(config: &LogConfig) {
    let mut builder = env_logger::Builder::new();
    builder
        .format_timestamp_millis()
        .filter_level(
            config.level.parse().unwrap_or(log::LevelFilter::Info),
        )
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_roundtrip() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.log.level, "info");
        assert_eq!(parsed.api.timeout_secs, 120);
        assert_eq!(parsed.storage.thumbnail_size, 256);
    }
}
