use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SelectorProfileType {
    #[default]
    Gui,
    Tui,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectorDefaults {
    pub gui: String,
    pub tui: String,
}

impl Default for SelectorDefaults {
    fn default() -> Self {
        Self {
            gui: "fuzzel".into(),
            tui: "fzf".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectorSettings {
    pub enable_selector: bool,
    #[serde(rename = "default")]
    pub defaults: SelectorDefaults,
    pub term_exec_args: Option<String>,
    pub expand_wildcards: bool,
}

impl Default for SelectorSettings {
    fn default() -> Self {
        Self {
            enable_selector: false,
            defaults: SelectorDefaults::default(),
            term_exec_args: Some("-e".into()),
            expand_wildcards: false,
        }
    }
}

impl SelectorSettings {
    pub fn default_for(&self, profile_type: SelectorProfileType) -> &str {
        match profile_type {
            SelectorProfileType::Gui => &self.defaults.gui,
            SelectorProfileType::Tui => &self.defaults.tui,
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
    pub selector_type: SelectorProfileType,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_launch_prefix: Option<String>,
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
                selector_type: SelectorProfileType::Tui,
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
                selector_type: SelectorProfileType::Gui,
            },
        );

        selector_profiles.insert(
            "rofi".to_string(),
            SelectorProfile {
                command: "rofi".to_string(),
                args: vec![
                    "-dmenu".to_string(),
                    "-p".to_string(),
                    "{prompt}".to_string(),
                ],
                env: HashMap::new(),
                entry_template: "{marker}{name}{comment}".to_string(),
                marker_default: Some("★".to_string()),
                marker_xdg: Some("▶".to_string()),
                marker_available: Some("   ".to_string()),
                prompt_template: None,
                header_template: None,
                selector_type: SelectorProfileType::Gui,
            },
        );

        Self {
            selector: SelectorSettings::default(),
            selector_profiles,
            marker_default: "★ ".to_string(),
            marker_xdg: "▶ ".to_string(),
            marker_available: "  ".to_string(),
            prompt_template: "Open '{file|truncate:20}' with: ".to_string(),
            header_template: "★=Default ▶=XDG Associated  =Available".to_string(),
            app_launch_prefix: None,
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
            .join("openit")
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

    pub fn selector_candidates(&self, preferred: SelectorProfileType) -> Vec<String> {
        let type_order = match preferred {
            SelectorProfileType::Gui => [SelectorProfileType::Gui, SelectorProfileType::Tui],
            SelectorProfileType::Tui => [SelectorProfileType::Tui, SelectorProfileType::Gui],
        };

        let mut candidates = Vec::new();

        for ty in type_order {
            let default_name = self.selector.default_for(ty).trim();
            if !default_name.is_empty()
                && !candidates.iter().any(|existing| existing == default_name)
            {
                candidates.push(default_name.to_string());
            }

            let mut names: Vec<String> = self
                .selector_profiles
                .iter()
                .filter_map(|(name, profile)| {
                    if profile.selector_type == ty {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();

            for name in names {
                if !candidates.contains(&name) {
                    candidates.push(name);
                }
            }
        }

        let mut remaining: Vec<String> = self
            .selector_profiles
            .keys()
            .filter(|name| !candidates.contains(*name))
            .cloned()
            .collect();
        remaining.sort();

        candidates.extend(remaining);

        candidates
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

        assert!(!config.selector.enable_selector);
        assert_eq!(config.selector.defaults.gui, "fuzzel");
        assert_eq!(config.selector.defaults.tui, "fzf");
        // Should have default fzf and fuzzel configs
        assert!(config.selector_profiles.contains_key("fzf"));
        assert!(config.selector_profiles.contains_key("fuzzel"));

        let fzf_config = config.get_selector_profile("fzf").unwrap();
        assert_eq!(fzf_config.selector_type, SelectorProfileType::Tui);

        assert_eq!(fzf_config.command, "fzf");
        assert!(fzf_config.args.contains(&"--reverse".to_string()));

        let fuzzel_config = config.get_selector_profile("fuzzel").unwrap();
        assert_eq!(fuzzel_config.selector_type, SelectorProfileType::Gui);

        assert!(config.app_launch_prefix.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let mut config = Config::default();
        config.app_launch_prefix = Some("flatpak run".to_string());

        // Test serialization
        let toml_string = toml::to_string(&config).unwrap();
        assert!(toml_string.contains("[selectors.fzf]"));
        assert!(toml_string.contains("app_launch_prefix = \"flatpak run\""));

        // Test deserialization
        let deserialized: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(
            config.selector_profiles.len(),
            deserialized.selector_profiles.len()
        );
        assert_eq!(config.app_launch_prefix, deserialized.app_launch_prefix);
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
            selector_type: SelectorProfileType::Gui,
        };

        // Test adding directly to the HashMap
        config
            .selector_profiles
            .insert("custom".to_string(), custom_config);

        assert!(config.selector_profiles.contains_key("custom"));
        assert_eq!(config.selector_profiles.len(), 4);
    }

    #[test]
    fn test_selector_candidates_preferred_order() {
        let config = Config::default();

        let gui_candidates = config.selector_candidates(SelectorProfileType::Gui);
        assert_eq!(gui_candidates.first().unwrap(), "fuzzel");
        assert!(gui_candidates.iter().any(|name| name == "fzf"));

        let tui_candidates = config.selector_candidates(SelectorProfileType::Tui);
        assert_eq!(tui_candidates.first().unwrap(), "fzf");
        assert!(tui_candidates.iter().any(|name| name == "fuzzel"));
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path();
        assert!(path.ends_with("openit/config.toml"));
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
