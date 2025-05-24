use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub mime_types: Vec<String>,
    pub no_display: bool,
    pub hidden: bool,
    pub terminal: bool,
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
            .with_context(|| format!("Failed to read desktop file: {:?}", path))?;

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
                } else if current_section.starts_with("[Desktop Action ") {
                    if !current_action.is_empty() {
                        if let Ok(action) = Self::build_desktop_action(&current_fields) {
                            actions.insert(current_action.clone(), action);
                        }
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

    fn build_desktop_entry(fields: &HashMap<String, String>) -> Result<DesktopEntry> {
        let name = fields
            .get("Name")
            .ok_or_else(|| anyhow::anyhow!("Missing Name field"))?
            .clone();

        let exec = fields
            .get("Exec")
            .ok_or_else(|| anyhow::anyhow!("Missing Exec field"))?
            .clone();

        let mime_types = fields
            .get("MimeType")
            .map(|s| {
                s.split(';')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let no_display = fields
            .get("NoDisplay")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(false);

        let hidden = fields
            .get("Hidden")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(false);

        let terminal = fields
            .get("Terminal")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(false);

        Ok(DesktopEntry {
            name,
            exec,
            comment: fields.get("Comment").cloned(),
            icon: fields.get("Icon").cloned(),
            mime_types,
            no_display,
            hidden,
            terminal,
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
        let content = r#"[Desktop Entry]
Name=Test App
Exec=testapp %F
Comment=A test application
Icon=test-icon
MimeType=text/plain;text/html;
Terminal=false
NoDisplay=false"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", content).unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(entry.name, "Test App");
        assert_eq!(entry.exec, "testapp %F");
        assert_eq!(entry.comment, Some("A test application".to_string()));
        assert_eq!(entry.icon, Some("test-icon".to_string()));
        assert_eq!(entry.mime_types, vec!["text/plain", "text/html"]);
        assert!(!entry.terminal);
        assert!(!entry.no_display);
    }

    #[test]
    fn test_parse_desktop_file_with_actions() {
        let content = r#"[Desktop Entry]
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
Exec=viewer --print %f"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", content).unwrap();

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
    fn test_parse_desktop_file_with_no_display() {
        let content = r#"[Desktop Entry]
Name=Hidden App
Exec=hidden
NoDisplay=true"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", content).unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert!(entry.no_display);
    }

    #[test]
    fn test_parse_desktop_file_missing_required_fields() {
        let content = r#"[Desktop Entry]
Comment=Missing name and exec"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", content).unwrap();

        let result = DesktopFile::parse(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_desktop_file() {
        let content = "";

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", content).unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        assert!(desktop_file.main_entry.is_none());
        assert!(desktop_file.actions.is_empty());
    }

    #[test]
    fn test_parse_desktop_file_with_comments() {
        let content = r#"# This is a comment
[Desktop Entry]
# Another comment
Name=Test App
Exec=test
# Comment in the middle
Icon=test-icon"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", content).unwrap();

        let desktop_file = DesktopFile::parse(temp_file.path()).unwrap();
        let entry = desktop_file.main_entry.unwrap();

        assert_eq!(entry.name, "Test App");
        assert_eq!(entry.exec, "test");
        assert_eq!(entry.icon, Some("test-icon".to_string()));
    }
}
