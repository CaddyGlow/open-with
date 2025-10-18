use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectorSettings {
    pub enable_selector: bool,
    pub selector: String,
    pub term_exec_args: Option<String>,
    pub expand_wildcards: bool,
}

impl Default for SelectorSettings {
    fn default() -> Self {
        Self {
            enable_selector: true,
            selector: "rofi -dmenu -i -p 'Open With: '".into(),
            term_exec_args: Some("-e".into()),
            expand_wildcards: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SelectorProfile {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub entry_template: String,
    pub marker_default: Option<String>,
    pub marker_xdg: Option<String>,
    pub marker_available: Option<String>,
    pub prompt_template: Option<String>,
    pub header_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub selector: SelectorSettings,
    #[serde(rename = "selectors")]
    pub selector_profiles: HashMap<String, SelectorProfile>,
    pub marker_default: String,
    pub marker_xdg: String,
    pub marker_available: String,
    pub prompt_template: String,
    pub header_template: String,
}

impl Default for Config {
    fn default() -> Self {
        let mut selector_profiles = HashMap::new();

        // Default fzf configuration
        selector_profiles.insert(
            "fzf".to_string(),
            SelectorProfile {
                command: "fzf".to_string(),
                args: vec![
                    "--prompt".to_string(),
                    "{prompt}".to_string(),
                    "--height=40%".to_string(),
                    "--reverse".to_string(),
                    "--header={header}".to_string(),
                    "--cycle".to_string(),
                ],
                env: HashMap::new(),
                entry_template: "{marker} {name}{comment}".to_string(),
                marker_default: None,
                marker_xdg: None,
                marker_available: None,
                prompt_template: None,
                header_template: None,
            },
        );

        // Default fuzzel configuration
        selector_profiles.insert(
            "fuzzel".to_string(),
            SelectorProfile {
                command: "fuzzel".to_string(),
                args: vec![
                    "--dmenu".to_string(),
                    "--prompt".to_string(),
                    "{prompt}".to_string(),
                    "--index".to_string(),
                    "--log-level=info".to_string(),
                ],
                env: HashMap::new(),
                entry_template: "{marker}{name}{comment}".to_string(),
                marker_default: Some("★".to_string()),
                marker_xdg: Some("▶".to_string()),
                marker_available: Some("   ".to_string()),
                prompt_template: None,
                header_template: None,
            },
        );

        Self {
            selector: SelectorSettings::default(),
            selector_profiles,
            marker_default: "★ ".to_string(),
            marker_xdg: "▶ ".to_string(),
            marker_available: "  ".to_string(),
            prompt_template: "Open '{file}' with: ".to_string(),
            header_template: "★=Default ▶=XDG Associated  =Available".to_string(),
        }
    }
}

impl Config {
    fn load_from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {}", path.display()))?;

        let config = toml::from_str::<Config>(&contents)
            .with_context(|| format!("Failed to parse config file at {}", path.display()))?;

        Ok(config)
    }

    pub fn load(custom_path: Option<PathBuf>) -> Result<Self> {
        if let Some(path) = custom_path {
            return Self::load_from_path(&path);
        }

        let config_path = Self::config_path();

        if config_path.exists() {
            if let Ok(config) = Self::load_from_path(&config_path) {
                return Ok(config);
            }
        }

        // Return default config if file doesn't exist or can't be parsed
        Ok(Self::default())
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_string = toml::to_string_pretty(self)?;
        fs::write(&config_path, toml_string)?;

        Ok(())
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("open-with")
            .join("config.toml")
    }

    pub fn handlr_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("handlr")
            .join("handlr.toml")
    }

    pub fn get_selector_profile(&self, name: &str) -> Option<&SelectorProfile> {
        self.selector_profiles.get(name)
    }

    pub fn get_marker<'a>(
        &'a self,
        selector_profile: &'a SelectorProfile,
        marker_type: &str,
    ) -> &'a str {
        match marker_type {
            "default" => selector_profile
                .marker_default
                .as_ref()
                .unwrap_or(&self.marker_default),
            "xdg" => selector_profile
                .marker_xdg
                .as_ref()
                .unwrap_or(&self.marker_xdg),
            "available" => selector_profile
                .marker_available
                .as_ref()
                .unwrap_or(&self.marker_available),
            _ => &self.marker_available,
        }
    }

    pub fn get_prompt_template<'a>(&'a self, selector_profile: &'a SelectorProfile) -> &'a str {
        selector_profile
            .prompt_template
            .as_ref()
            .unwrap_or(&self.prompt_template)
    }

    pub fn get_header_template<'a>(&'a self, selector_profile: &'a SelectorProfile) -> &'a str {
        selector_profile
            .header_template
            .as_ref()
            .unwrap_or(&self.header_template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert!(config.selector.enable_selector);
        assert_eq!(config.selector.selector, "rofi -dmenu -i -p 'Open With: '");
        // Should have default fzf and fuzzel configs
        assert!(config.selector_profiles.contains_key("fzf"));
        assert!(config.selector_profiles.contains_key("fuzzel"));

        let fzf_config = config.get_selector_profile("fzf").unwrap();
        assert_eq!(fzf_config.command, "fzf");
        assert!(fzf_config.args.contains(&"--reverse".to_string()));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();

        // Test serialization
        let toml_string = toml::to_string(&config).unwrap();
        assert!(toml_string.contains("[selectors.fzf]"));

        // Test deserialization
        let deserialized: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(
            config.selector_profiles.len(),
            deserialized.selector_profiles.len()
        );
    }

    #[test]
    fn test_add_custom_fuzzy_finder() {
        let mut config = Config::default();

        let custom_config = SelectorProfile {
            command: "custom-fuzzy".to_string(),
            args: vec!["--custom-arg".to_string()],
            env: HashMap::new(),
            entry_template: "{marker} {name}{comment}".to_string(),
            marker_default: None,
            marker_xdg: None,
            marker_available: None,
            prompt_template: None,
            header_template: None,
        };

        // Test adding directly to the HashMap
        config
            .selector_profiles
            .insert("custom".to_string(), custom_config);

        assert!(config.selector_profiles.contains_key("custom"));
        assert_eq!(config.selector_profiles.len(), 3);
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path();
        assert!(path.ends_with("open-with/config.toml"));
    }

    #[test]
    fn test_load_custom_config_success() {
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("config.toml");

        let original = Config::default();
        let contents = toml::to_string_pretty(&original).unwrap();
        fs::write(&custom_path, contents).unwrap();

        let loaded = Config::load(Some(custom_path)).unwrap();
        assert_eq!(
            loaded.selector_profiles.len(),
            original.selector_profiles.len()
        );
    }

    #[test]
    fn test_load_custom_config_missing_file_errors() {
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("missing_config.toml");

        let err = Config::load(Some(custom_path.clone())).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Failed to read config file"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn test_load_custom_config_invalid_file_errors() {
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("invalid.toml");
        fs::write(&custom_path, "not = [valid").unwrap();

        let err = Config::load(Some(custom_path.clone())).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Failed to parse config file"),
            "unexpected error message: {message}"
        );
    }
}
