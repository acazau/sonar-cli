use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StoredConfig {
    pub url: Option<String>,
    pub token: Option<String>,
}

/// Returns the path to the config file: `<config_dir>/sonar-cli/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("sonar-cli").join("config.toml"))
}

/// Load config from the default path. Returns default if missing or malformed.
pub fn load() -> StoredConfig {
    match config_path() {
        Some(p) => load_from(&p),
        None => {
            tracing::warn!("Could not determine config directory");
            StoredConfig::default()
        }
    }
}

/// Save config to the default path. Creates parent directories as needed.
pub fn save(config: &StoredConfig) -> Result<(), String> {
    match config_path() {
        Some(p) => save_to(config, &p),
        None => Err("Could not determine config directory".to_string()),
    }
}

/// Remove the config file. No-op if it does not exist.
pub fn remove() -> Result<(), String> {
    match config_path() {
        Some(p) => remove_at(&p),
        None => Err("Could not determine config directory".to_string()),
    }
}

fn load_from(path: &PathBuf) -> StoredConfig {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("Malformed config at {}: {e}", path.display());
                StoredConfig::default()
            }
        },
        Err(_) => StoredConfig::default(),
    }
}

fn save_to(config: &StoredConfig, path: &PathBuf) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
    }
    let contents = toml::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(path, contents).map_err(|e| format!("Failed to write config file: {e}"))
}

fn remove_at(path: &PathBuf) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Failed to remove config file: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_nonexistent_returns_default() {
        let dir = std::env::temp_dir().join("sonar-cli-test-load-nonexistent");
        let path = dir.join("config.toml");
        let _ = std::fs::remove_file(&path);

        let cfg = load_from(&path);
        assert!(cfg.url.is_none());
        assert!(cfg.token.is_none());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("sonar-cli-test-roundtrip");
        let path = dir.join("config.toml");

        let config = StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abc123".to_string()),
        };
        save_to(&config, &path).unwrap();

        let loaded = load_from(&path);
        assert_eq!(loaded.url.as_deref(), Some("https://sonar.example.com"));
        assert_eq!(loaded.token.as_deref(), Some("squ_abc123"));

        // cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_malformed_returns_default() {
        let dir = std::env::temp_dir().join("sonar-cli-test-malformed");
        let path = dir.join("config.toml");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(&path, "this is not valid toml {{{{").unwrap();

        let cfg = load_from(&path);
        assert!(cfg.url.is_none());
        assert!(cfg.token.is_none());

        // cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_remove_nonexistent_succeeds() {
        let dir = std::env::temp_dir().join("sonar-cli-test-remove-nonexistent");
        let path = dir.join("config.toml");
        let _ = std::fs::remove_file(&path);

        assert!(remove_at(&path).is_ok());
    }
}
