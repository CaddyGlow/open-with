use crate::config::Config;
use anyhow::{Context, Result};
use log::debug;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RegexHandlersFile {
    #[serde(default)]
    handlers: Vec<RegexHandlerDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RegexHandlerDefinition {
    pub exec: String,
    pub regexes: Vec<String>,
    pub terminal: bool,
    pub priority: i32,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegexHandler {
    #[allow(dead_code)]
    pub exec: String,
    #[allow(dead_code)]
    pub terminal: bool,
    pub priority: i32,
    #[allow(dead_code)]
    pub notes: Option<String>,
    #[allow(dead_code)]
    patterns: Vec<String>,
    compiled: Vec<Regex>,
}

impl RegexHandler {
    #[allow(dead_code)]
    pub fn matches(&self, candidate: &str) -> bool {
        self.compiled.iter().any(|regex| regex.is_match(candidate))
    }

    #[allow(dead_code)]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

#[derive(Debug, Clone)]
pub struct RegexHandlerStore {
    #[allow(dead_code)]
    definitions: Vec<RegexHandlerDefinition>,
    handlers: Vec<RegexHandler>,
}

impl RegexHandlerStore {
    pub fn load(custom_path: Option<PathBuf>) -> Result<Self> {
        let path = custom_path.unwrap_or_else(Self::config_path);

        if path.exists() {
            let contents = fs::read_to_string(&path).with_context(|| {
                format!("Failed to read regex handler file at {}", path.display())
            })?;
            let file: RegexHandlersFile = toml::from_str(&contents).with_context(|| {
                format!("Failed to parse regex handler file at {}", path.display())
            })?;
            return Self::from_definitions(file.handlers);
        }

        if let Some(handlers) = Self::load_handlr_handlers()? {
            return Self::from_definitions(handlers);
        }

        Ok(Self {
            definitions: Vec::new(),
            handlers: Vec::new(),
        })
    }

    #[allow(dead_code)]
    pub fn save(&self, custom_path: Option<PathBuf>) -> Result<()> {
        let path = custom_path.unwrap_or_else(Self::config_path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = RegexHandlersFile {
            handlers: self.definitions.clone(),
        };

        let contents = toml::to_string_pretty(&file)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openit")
            .join("regex_handlers.toml")
    }

    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    #[allow(dead_code)]
    pub fn handlers(&self) -> &[RegexHandler] {
        &self.handlers
    }

    #[allow(dead_code)]
    pub fn find_handler(&self, candidate: &str) -> Option<&RegexHandler> {
        self.handlers
            .iter()
            .find(|handler| handler.matches(candidate))
    }

    fn from_definitions(definitions: Vec<RegexHandlerDefinition>) -> Result<Self> {
        let mut compiled_handlers = Vec::new();

        for definition in &definitions {
            let mut compiled_patterns = Vec::new();
            for pattern in &definition.regexes {
                let regex = Regex::new(pattern).with_context(|| {
                    format!(
                        "Failed to compile regex `{pattern}` for handler `{}`",
                        definition.exec
                    )
                })?;
                compiled_patterns.push(regex);
            }

            compiled_handlers.push(RegexHandler {
                exec: definition.exec.clone(),
                terminal: definition.terminal,
                priority: definition.priority,
                notes: definition.notes.clone(),
                patterns: definition.regexes.clone(),
                compiled: compiled_patterns,
            });
        }

        compiled_handlers.sort_by(|a, b| b.priority.cmp(&a.priority));

        debug!("Loaded {} regex handler(s)", compiled_handlers.len());

        Ok(Self {
            definitions,
            handlers: compiled_handlers,
        })
    }

    fn load_handlr_handlers() -> Result<Option<Vec<RegexHandlerDefinition>>> {
        let handlr_path = Config::handlr_config_path();
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

        let file: RegexHandlersFile = match toml::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                debug!(
                    "Failed to parse handlr config at {}: {}",
                    handlr_path.display(),
                    err
                );
                return Ok(None);
            }
        };

        if file.handlers.is_empty() {
            return Ok(None);
        }

        debug!(
            "Loaded {} regex handler(s) from handlr config",
            file.handlers.len()
        );
        Ok(Some(file.handlers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_load_empty_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("regex_handlers.toml");

        let store = RegexHandlerStore::load(Some(path)).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn test_load_and_match_handler() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"
[[handlers]]
exec = "xdg-open %u"
regexes = ["https://.*"]
terminal = false
priority = 5
notes = "Open HTTPS URLs"
"#
        )
        .unwrap();

        let store = RegexHandlerStore::load(Some(file.path().to_path_buf())).unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(store.handlers().len(), 1);

        let handler = store.find_handler("https://example.com").unwrap();
        assert_eq!(handler.exec, "xdg-open %u");
        assert_eq!(handler.priority, 5);
        assert_eq!(handler.notes.as_deref(), Some("Open HTTPS URLs"));
        assert_eq!(handler.patterns().len(), 1);
        assert!(!handler.terminal);
        assert!(handler.matches("https://example.com"));
        assert!(!handler.matches("http://example.com"));
    }

    #[test]
    fn test_invalid_regex_returns_error() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"
[[handlers]]
exec = "xdg-open %u"
regexes = ["[invalid"]
"#
        )
        .unwrap();

        let err = RegexHandlerStore::load(Some(file.path().to_path_buf())).unwrap_err();
        assert!(err.to_string().contains("Failed to compile regex"));
    }

    #[test]
    fn test_save_round_trip() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"
[[handlers]]
exec = "xdg-open %u"
regexes = ["https://.*"]
priority = 1

[[handlers]]
exec = "vlc %u"
regexes = ["https://youtu\\.be/.*"]
priority = 10
"#
        )
        .unwrap();

        let store = RegexHandlerStore::load(Some(file.path().to_path_buf())).unwrap();
        assert_eq!(store.len(), 2);

        let temp_dir = TempDir::new().unwrap();
        let save_path = temp_dir.path().join("handlers.toml");
        store.save(Some(save_path.clone())).unwrap();

        let reloaded = RegexHandlerStore::load(Some(save_path)).unwrap();
        assert_eq!(reloaded.len(), 2);
        assert_eq!(reloaded.handlers().len(), 2);
        assert!(!reloaded.handlers()[0].patterns().is_empty());
        assert!(reloaded
            .find_handler("https://youtu.be/dQw4w9WgXcQ")
            .is_some());
    }
}
