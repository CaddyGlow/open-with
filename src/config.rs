use anyhow::{Context, Result};
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectorConfig {
    pub enable_selector: bool,
    pub selector: String,
    pub term_exec_args: Option<String>,
    pub expand_wildcards: bool,
}

impl Default for SelectorConfig {
    fn default() -> Self {
        Self {
            enable_selector: false,
            selector: "rofi -dmenu -i -p 'Open With: '".into(),
            term_exec_args: Some("-e".into()),
            expand_wildcards: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FuzzyFinderConfig {
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

impl Default for FuzzyFinderConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            entry_template: String::new(),
            marker_default: None,
            marker_xdg: None,
            marker_available: None,
            prompt_template: None,
            header_template: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub selector: SelectorConfig,
    pub fuzzy_finders: HashMap<String, FuzzyFinderConfig>,
    pub marker_default: String,
    pub marker_xdg: String,
    pub marker_available: String,
    pub prompt_template: String,
    pub header_template: String,
}

impl Default for Config {
    fn default() -> Self {
        let mut fuzzy_finders = HashMap::new();

        // Default fzf configuration
        fuzzy_finders.insert(
            "fzf".to_string(),
            FuzzyFinderConfig {
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
        fuzzy_finders.insert(
            "fuzzel".to_string(),
            FuzzyFinderConfig {
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
            selector: SelectorConfig::default(),
            fuzzy_finders,
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

    fn load_handlr_selector_config() -> Result<Option<SelectorConfig>> {
        let handlr_path = Self::handlr_config_path();

        if !handlr_path.exists() {
            return Ok(None);
        }

        let contents = match fs::read_to_string(&handlr_path) {
            Ok(contents) => contents,
            Err(err) => {
                debug!(
                    "Failed to read handlr config at {}: {}",
                    handlr_path.display(),
                    err
                );
                return Ok(None);
            }
        };

        match toml::from_str::<HandlrCompatConfig>(&contents) {
            Ok(raw) => Ok(Some(raw.into_selector_config())),
            Err(err) => {
                debug!(
                    "Failed to parse handlr config at {}: {}",
                    handlr_path.display(),
                    err
                );
                Ok(None)
            }
        }
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

        if let Some(selector) = Self::load_handlr_selector_config()? {
            let mut config = Self::default();
            config.selector = selector;
            return Ok(config);
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

    pub fn get_fuzzy_finder(&self, name: &str) -> Option<&FuzzyFinderConfig> {
        self.fuzzy_finders.get(name)
    }

    pub fn get_marker<'a>(
        &'a self,
        fuzzer_config: &'a FuzzyFinderConfig,
        marker_type: &str,
    ) -> &'a str {
        match marker_type {
            "default" => fuzzer_config
                .marker_default
                .as_ref()
                .unwrap_or(&self.marker_default),
            "xdg" => fuzzer_config
                .marker_xdg
                .as_ref()
                .unwrap_or(&self.marker_xdg),
            "available" => fuzzer_config
                .marker_available
                .as_ref()
                .unwrap_or(&self.marker_available),
            _ => &self.marker_available,
        }
    }

    pub fn get_prompt_template<'a>(&'a self, fuzzer_config: &'a FuzzyFinderConfig) -> &'a str {
        fuzzer_config
            .prompt_template
            .as_ref()
            .unwrap_or(&self.prompt_template)
    }

    pub fn get_header_template<'a>(&'a self, fuzzer_config: &'a FuzzyFinderConfig) -> &'a str {
        fuzzer_config
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

        assert!(!config.selector.enable_selector);
        assert_eq!(config.selector.selector, "rofi -dmenu -i -p 'Open With: '");
        // Should have default fzf and fuzzel configs
        assert!(config.fuzzy_finders.contains_key("fzf"));
        assert!(config.fuzzy_finders.contains_key("fuzzel"));

        let fzf_config = config.get_fuzzy_finder("fzf").unwrap();
        assert_eq!(fzf_config.command, "fzf");
        assert!(fzf_config.args.contains(&"--reverse".to_string()));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();

        // Test serialization
        let toml_string = toml::to_string(&config).unwrap();
        assert!(toml_string.contains("[fuzzy_finders.fzf]"));

        // Test deserialization
        let deserialized: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(config.fuzzy_finders.len(), deserialized.fuzzy_finders.len());
    }

    #[test]
    fn test_add_custom_fuzzy_finder() {
        let mut config = Config::default();

        let custom_config = FuzzyFinderConfig {
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
            .fuzzy_finders
            .insert("custom".to_string(), custom_config);

        assert!(config.fuzzy_finders.contains_key("custom"));
        assert_eq!(config.fuzzy_finders.len(), 3);
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
        assert_eq!(loaded.fuzzy_finders.len(), original.fuzzy_finders.len());
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

    #[test]
    fn test_fallback_to_handlr_config() {
        use std::env;
        use std::ffi::OsString;

        struct ConfigHomeGuard {
            original: Option<OsString>,
        }

        impl ConfigHomeGuard {
            fn set(path: &Path) -> Self {
                let original = env::var_os("XDG_CONFIG_HOME");
                env::set_var("XDG_CONFIG_HOME", path);
                Self { original }
            }
        }

        impl Drop for ConfigHomeGuard {
            fn drop(&mut self) {
                if let Some(original) = self.original.take() {
                    env::set_var("XDG_CONFIG_HOME", original);
                } else {
                    env::remove_var("XDG_CONFIG_HOME");
                }
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let _guard = ConfigHomeGuard::set(&config_dir);

        let handlr_dir = config_dir.join("handlr");
        fs::create_dir_all(&handlr_dir).unwrap();
        let handlr_path = handlr_dir.join("handlr.toml");
        let handlr_contents = r#"
enable_selector = true
selector = "wofi -dmenu"
term_exec_args = "-x"
expand_wildcards = true

[[handlers]]
exec = "dummy %f"
regexes = ["foo"]
"#;
        fs::write(&handlr_path, handlr_contents).unwrap();

        let config = Config::load(None).unwrap();
        assert!(config.selector.enable_selector);
        assert_eq!(config.selector.selector, "wofi -dmenu");
        assert_eq!(config.selector.term_exec_args.as_deref(), Some("-x"));
        assert!(config.selector.expand_wildcards);
        assert!(config.get_fuzzy_finder("fzf").is_some());
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct HandlrCompatConfig {
    enable_selector: bool,
    selector: String,
    term_exec_args: Option<String>,
    expand_wildcards: bool,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

impl Default for HandlrCompatConfig {
    fn default() -> Self {
        let selector = SelectorConfig::default();
        Self {
            enable_selector: selector.enable_selector,
            selector: selector.selector,
            term_exec_args: selector.term_exec_args,
            expand_wildcards: selector.expand_wildcards,
            _extra: HashMap::new(),
        }
    }
}

impl HandlrCompatConfig {
    fn into_selector_config(self) -> SelectorConfig {
        SelectorConfig {
            enable_selector: self.enable_selector,
            selector: self.selector,
            term_exec_args: self.term_exec_args,
            expand_wildcards: self.expand_wildcards,
        }
    }
}
