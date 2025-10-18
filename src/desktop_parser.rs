use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntry {
    #[serde(default = "DesktopEntry::default_entry_type")]
    pub entry_type: String,
    #[serde(default)]
    pub version: Option<String>,
    pub name: String,
    #[serde(default)]
    pub generic_name: Option<String>,
    pub exec: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub no_display: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub only_show_in: Vec<String>,
    #[serde(default)]
    pub not_show_in: Vec<String>,
    #[serde(default)]
    pub dbus_activatable: bool,
    #[serde(default)]
    pub try_exec: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub mime_types: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub implements: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub startup_notify: bool,
    #[serde(default)]
    pub startup_wm_class: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub prefers_non_default_gpu: bool,
    #[serde(default)]
    pub single_main_window: bool,
    #[serde(default)]
    pub actions: Vec<String>,
}

impl DesktopEntry {
    fn default_entry_type() -> String {
        "Application".to_string()
    }
}

impl Default for DesktopEntry {
    fn default() -> Self {
        Self {
            entry_type: DesktopEntry::default_entry_type(),
            version: None,
            name: String::new(),
            generic_name: None,
            exec: String::new(),
            comment: None,
            icon: None,
            no_display: false,
            hidden: false,
            only_show_in: Vec::new(),
            not_show_in: Vec::new(),
            dbus_activatable: false,
            try_exec: None,
            path: None,
            terminal: false,
            mime_types: Vec::new(),
            categories: Vec::new(),
            implements: Vec::new(),
            keywords: Vec::new(),
            startup_notify: false,
            startup_wm_class: None,
            url: None,
            prefers_non_default_gpu: false,
            single_main_window: false,
            actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopAction {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopFile {
    pub main_entry: Option<DesktopEntry>,
    pub actions: HashMap<String, DesktopAction>,
}

impl DesktopFile {
    pub fn parse(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read desktop file: {}", path.display()))?;

        let mut main_entry = None;
        let mut actions = HashMap::new();
        let mut current_section = String::new();
        let mut current_action = String::new();

        // Temporary storage for current section being parsed
        let mut current_fields: HashMap<String, String> = HashMap::new();

        for line in contents.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Check for section headers
            if line.starts_with('[') && line.ends_with(']') {
                // Save previous section if needed
                if current_section == "[Desktop Entry]" {
                    main_entry = Some(Self::build_desktop_entry(&current_fields)?);
                } else if current_section.starts_with("[Desktop Action ")
                    && !current_action.is_empty()
                {
                    if let Ok(action) = Self::build_desktop_action(&current_fields) {
                        actions.insert(current_action.clone(), action);
                    }
                }

                // Start new section
                current_section = line.to_string();
                current_fields.clear();

                if current_section.starts_with("[Desktop Action ") {
                    current_action = current_section
                        .trim_start_matches("[Desktop Action ")
                        .trim_end_matches(']')
                        .to_string();
                }
                continue;
            }

            // Parse key=value pairs
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();
                current_fields.insert(key.to_string(), value.to_string());
            }
        }

        // Handle last section
        if current_section == "[Desktop Entry]" {
            main_entry = Some(Self::build_desktop_entry(&current_fields)?);
        } else if current_section.starts_with("[Desktop Action ") && !current_action.is_empty() {
            if let Ok(action) = Self::build_desktop_action(&current_fields) {
                actions.insert(current_action, action);
            }
        }

        Ok(DesktopFile {
            main_entry,
            actions,
        })
    }

    fn parse_bool(value: Option<&String>) -> bool {
        value
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.eq_ignore_ascii_case("true") {
                    true
                } else if s.eq_ignore_ascii_case("false") {
                    false
                } else {
                    false
                }
            })
            .unwrap_or(false)
    }

    fn parse_list(value: Option<&String>) -> Vec<String> {
        value
            .map(|s| {
                s.split(';')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(std::string::ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn parse_optional_string(value: Option<&String>) -> Option<String> {
        value.and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
    }

    fn build_desktop_entry(fields: &HashMap<String, String>) -> Result<DesktopEntry> {
        let entry_type = Self::parse_optional_string(fields.get("Type"))
            .unwrap_or_else(DesktopEntry::default_entry_type);

        let name = Self::parse_optional_string(fields.get("Name"))
            .ok_or_else(|| anyhow::anyhow!("Missing Name field"))?;

        let exec = fields
            .get("Exec")
            .ok_or_else(|| anyhow::anyhow!("Missing Exec field"))?
            .clone();

        if exec.trim().is_empty() {
            anyhow::bail!("Missing Exec field");
        }

        let version = Self::parse_optional_string(fields.get("Version"));
        let generic_name = Self::parse_optional_string(fields.get("GenericName"));
        let comment = Self::parse_optional_string(fields.get("Comment"));
        let icon = Self::parse_optional_string(fields.get("Icon"));

        let mime_types = Self::parse_list(fields.get("MimeType"));

        let no_display = Self::parse_bool(fields.get("NoDisplay"));
        let hidden = Self::parse_bool(fields.get("Hidden"));
        let only_show_in = Self::parse_list(fields.get("OnlyShowIn"));
        let not_show_in = Self::parse_list(fields.get("NotShowIn"));
        let dbus_activatable = Self::parse_bool(fields.get("DBusActivatable"));
        let try_exec = Self::parse_optional_string(fields.get("TryExec"));
        let path = Self::parse_optional_string(fields.get("Path"));
        let terminal = Self::parse_bool(fields.get("Terminal"));
        let categories = Self::parse_list(fields.get("Categories"));
        let implements = Self::parse_list(fields.get("Implements"));
        let keywords = Self::parse_list(fields.get("Keywords"));
        let startup_notify = Self::parse_bool(fields.get("StartupNotify"));
        let startup_wm_class = Self::parse_optional_string(fields.get("StartupWMClass"));
        let url = Self::parse_optional_string(fields.get("URL"));
        let prefers_non_default_gpu = Self::parse_bool(fields.get("PrefersNonDefaultGPU"));
        let single_main_window = Self::parse_bool(fields.get("SingleMainWindow"));
        let actions = Self::parse_list(fields.get("Actions"));

        Ok(DesktopEntry {
            entry_type,
            version,
            name,
            exec,
            generic_name,
            comment,
            icon,
            no_display,
            hidden,
            only_show_in,
            not_show_in,
            dbus_activatable,
            try_exec,
            path,
            terminal,
            mime_types,
            categories,
            implements,
            keywords,
            startup_notify,
            startup_wm_class,
            url,
            prefers_non_default_gpu,
            single_main_window,
            actions,
        })
    }

    fn build_desktop_action(fields: &HashMap<String, String>) -> Result<DesktopAction> {
        let name = fields
            .get("Name")
            .ok_or_else(|| anyhow::anyhow!("Missing Name field in action"))?
            .clone();

        let exec = fields
            .get("Exec")
            .ok_or_else(|| anyhow::anyhow!("Missing Exec field in action"))?
            .clone();

        Ok(DesktopAction {
            name,
            exec,
            icon: fields.get("Icon").cloned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_desktop_file() {
        let content = r"[Desktop Entry]
Name=Test App
Exec=testapp %F
Comment=A test application
Icon=test-icon
MimeType=text/plain;text/html;
Terminal=false
NoDisplay=false";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(entry.name, "Test App");
        assert_eq!(entry.exec, "testapp %F");
        assert_eq!(entry.comment, Some("A test application".to_string()));
        assert_eq!(entry.icon, Some("test-icon".to_string()));
        assert_eq!(entry.mime_types, vec!["text/plain", "text/html"]);
        assert!(!entry.terminal);
        assert!(!entry.no_display);
        assert!(entry.categories.is_empty());
    }

    #[test]
    fn test_parse_desktop_file_with_actions() {
        let content = r"[Desktop Entry]
Name=Image Viewer
Exec=viewer %f
Icon=viewer
MimeType=image/png;image/jpeg;
Actions=edit;print;

[Desktop Action edit]
Name=Edit Image
Exec=viewer --edit %f
Icon=edit-icon

[Desktop Action print]
Name=Print Image
Exec=viewer --print %f";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();

        // Check main entry
        let entry = desktop_file.main_entry.as_ref().unwrap();
        assert_eq!(entry.name, "Image Viewer");
        assert_eq!(entry.mime_types, vec!["image/png", "image/jpeg"]);

        // Check actions
        assert_eq!(desktop_file.actions.len(), 2);

        let edit_action = desktop_file.actions.get("edit").unwrap();
        assert_eq!(edit_action.name, "Edit Image");
        assert_eq!(edit_action.exec, "viewer --edit %f");
        assert_eq!(edit_action.icon, Some("edit-icon".to_string()));

        let print_action = desktop_file.actions.get("print").unwrap();
        assert_eq!(print_action.name, "Print Image");
        assert_eq!(print_action.exec, "viewer --print %f");
        assert_eq!(print_action.icon, None);
    }

    #[test]
    fn test_parse_desktop_file_with_categories() {
        let content = r"[Desktop Entry]
Name=Terminal App
Exec=terminal --new
Categories=Utility;TerminalEmulator;System;
Terminal=false";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(
            entry.categories,
            vec!["Utility", "TerminalEmulator", "System"]
        );
    }

    #[test]
    fn test_parse_desktop_file_with_no_display() {
        let content = r"[Desktop Entry]
Name=Hidden App
Exec=hidden
NoDisplay=true";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert!(entry.no_display);
    }

    #[test]
    fn test_parse_desktop_file_missing_required_fields() {
        let content = r"[Desktop Entry]
Comment=Missing name and exec";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let result = DesktopFile::parse(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_desktop_file() {
        let content = "";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        assert!(desktop_file.main_entry.is_none());
        assert!(desktop_file.actions.is_empty());
    }

    #[test]
    fn test_parse_desktop_file_with_comments() {
        let content = r" This is a comment
[Desktop Entry]
# Another comment
Name=Test App
Exec=test
# Comment in the middle
Icon=test-icon";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(entry.name, "Test App");
        assert_eq!(entry.exec, "test");
        assert_eq!(entry.icon, Some("test-icon".to_string()));
    }

    #[test]
    fn test_parse_desktop_file_with_hidden() {
        let content = r"[Desktop Entry]
Name=Hidden App
Exec=hidden
Hidden=true";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert!(entry.hidden);
    }

    #[test]
    fn test_parse_desktop_file_with_terminal() {
        let content = r"[Desktop Entry]
Name=Terminal App
Exec=terminal-app
Terminal=true";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert!(entry.terminal);
    }

    #[test]
    fn test_parse_invalid_file_path() {
        let result = DesktopFile::parse(Path::new("/nonexistent/file.desktop"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read desktop file"));
    }

    #[test]
    fn test_parse_desktop_file_action_without_exec() {
        let content = r"[Desktop Entry]
Name=App
Exec=app
Actions=broken;

[Desktop Action broken]
Name=Broken Action";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();

        // Main entry should parse fine
        assert!(desktop_file.main_entry.is_some());

        // Action without Exec should not be included
        assert!(!desktop_file.actions.contains_key("broken"));
    }

    #[test]
    fn test_parse_desktop_file_with_equals_in_value() {
        let content = r"[Desktop Entry]
Name=Test App
Exec=test --option=value
Comment=Test=Application
Icon=test-icon";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(entry.name, "Test App");
        assert_eq!(entry.exec, "test --option=value");
        assert_eq!(entry.comment, Some("Test=Application".to_string()));
    }

    #[test]
    fn test_build_desktop_action_missing_name() {
        let mut fields = HashMap::new();
        fields.insert("Exec".to_string(), "test".to_string());

        let result = DesktopFile::build_desktop_action(&fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing Name field"));
    }

    #[test]
    fn test_parse_desktop_file_with_multiple_sections() {
        let content = r"[Desktop Entry]
Name=Multi Section App
Exec=app

[Desktop Action one]
Name=Action One
Exec=app --one

[Some Other Section]
Key=Value

[Desktop Action two]
Name=Action Two
Exec=app --two";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();

        // Should have main entry
        assert!(desktop_file.main_entry.is_some());
        assert_eq!(
            desktop_file.main_entry.as_ref().unwrap().name,
            "Multi Section App"
        );

        // Should have both actions
        assert_eq!(desktop_file.actions.len(), 2);
        assert!(desktop_file.actions.contains_key("one"));
        assert!(desktop_file.actions.contains_key("two"));

        // Other sections should be ignored
    }

    #[test]
    fn test_parse_desktop_file_last_section_is_action() {
        let content = r"[Desktop Entry]
Name=App
Exec=app

[Desktop Action last]
Name=Last Action
Exec=app --last";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();

        // Should handle last section being an action
        assert_eq!(desktop_file.actions.len(), 1);
        assert!(desktop_file.actions.contains_key("last"));
    }

    #[test]
    fn test_parse_desktop_file_with_recognized_keys() {
        let content = r"[Desktop Entry]
Type=Application
Version=1.2
Name=Full Featured App
GenericName=Full App
NoDisplay=true
Comment=Full featured application
Icon=full-app
Hidden=true
OnlyShowIn=GNOME;KDE;
NotShowIn=XFCE;
DBusActivatable=true
TryExec=/usr/bin/full
Exec=full %F
Path=/opt/full
Terminal=true
MimeType=text/plain;text/html;
Categories=Utility;Office;
Implements=org.example.Full;org.example.Advanced;
Keywords=full;featured;app;
StartupNotify=true
StartupWMClass=FullAppClass
URL=https://example.com
PrefersNonDefaultGPU=true
SingleMainWindow=true
Actions=Edit;View;

[Desktop Action Edit]
Name=Edit Document
Exec=full --edit %F

[Desktop Action View]
Name=View Document
Exec=full --view %F";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{content}").unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(entry.entry_type, "Application");
        assert_eq!(entry.version.as_deref(), Some("1.2"));
        assert_eq!(entry.generic_name.as_deref(), Some("Full App"));
        assert!(entry.no_display);
        assert_eq!(entry.comment.as_deref(), Some("Full featured application"));
        assert_eq!(entry.icon.as_deref(), Some("full-app"));
        assert!(entry.hidden);
        assert_eq!(entry.only_show_in, vec!["GNOME", "KDE"]);
        assert_eq!(entry.not_show_in, vec!["XFCE"]);
        assert!(entry.dbus_activatable);
        assert_eq!(entry.try_exec.as_deref(), Some("/usr/bin/full"));
        assert_eq!(entry.path.as_deref(), Some("/opt/full"));
        assert!(entry.terminal);
        assert_eq!(entry.mime_types, vec!["text/plain", "text/html"]);
        assert_eq!(entry.categories, vec!["Utility", "Office"]);
        assert_eq!(
            entry.implements,
            vec!["org.example.Full", "org.example.Advanced"]
        );
        assert_eq!(entry.keywords, vec!["full", "featured", "app"]);
        assert!(entry.startup_notify);
        assert_eq!(entry.startup_wm_class.as_deref(), Some("FullAppClass"));
        assert_eq!(entry.url.as_deref(), Some("https://example.com"));
        assert!(entry.prefers_non_default_gpu);
        assert!(entry.single_main_window);
        assert_eq!(entry.actions, vec!["Edit", "View"]);

        assert_eq!(desktop_file.actions.len(), 2);
        assert!(desktop_file.actions.contains_key("Edit"));
        assert!(desktop_file.actions.contains_key("View"));
    }
}
