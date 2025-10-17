use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
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
    pub fn load(custom_path: Option<PathBuf>) -> Self {
        let config_path = custom_path.unwrap_or_else(Self::config_path);

        if config_path.exists() {
            if let Ok(contents) = fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str::<Config>(&contents) {
                    return config;
                }
            }
        }

        // Return default config if file doesn't exist or can't be parsed
        Self::default()
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

    #[test]
    fn test_default_config() {
        let config = Config::default();

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
}
