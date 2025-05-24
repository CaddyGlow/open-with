use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct MimeAssociations {
    associations: HashMap<String, Vec<String>>,
}

impl MimeAssociations {
    pub fn new() -> Self {
        Self {
            associations: HashMap::new(),
        }
    }

    // Also add this for testing
    #[cfg(test)]
    pub fn with_associations(associations: HashMap<String, Vec<String>>) -> Self {
        Self { associations }
    }
    pub fn load() -> Result<Self> {
        let mut associations = HashMap::new();
        let mimeapps_files = crate::xdg::get_mimeapps_list_files();

        // Process files in reverse order (later files override earlier ones)
        for file in mimeapps_files.iter().rev() {
            if let Ok(contents) = fs::read_to_string(file) {
                Self::parse_mimeapps_file(&contents, &mut associations);
            }
        }

        Ok(Self { associations })
    }

    fn parse_mimeapps_file(contents: &str, associations: &mut HashMap<String, Vec<String>>) {
        let mut current_section = String::new();

        for line in contents.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line.to_string();
                continue;
            }

            if current_section == "[Default Applications]"
                || current_section == "[Added Associations]"
            {
                if let Some(eq_pos) = line.find('=') {
                    let mime_type = line[..eq_pos].trim();
                    let apps = line[eq_pos + 1..]
                        .split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.trim().to_string())
                        .collect::<Vec<_>>();

                    if !apps.is_empty() {
                        // For Default Applications, replace existing
                        // For Added Associations, append
                        if current_section == "[Default Applications]" {
                            associations.insert(mime_type.to_string(), apps);
                        } else {
                            associations
                                .entry(mime_type.to_string())
                                .or_insert_with(Vec::new)
                                .extend(apps);
                        }
                    }
                }
            }
        }
    }

    pub fn get_associations(&self, mime_type: &str) -> Vec<String> {
        self.associations
            .get(mime_type)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_parse_mimeapps_file_default_applications() {
        let mut associations = HashMap::new();
        let content = r#"[Default Applications]
text/plain=editor.desktop;notepad.desktop;
image/png=viewer.desktop;

[Added Associations]
text/plain=extra-editor.desktop;"#;

        MimeAssociations::parse_mimeapps_file(&content, &mut associations);

        // The function processes both Default and Added sections
        // Since Default Applications comes first, it sets the initial value
        // Then Added Associations appends to it
        let text_apps = associations.get("text/plain").unwrap();
        assert_eq!(
            text_apps,
            &vec!["editor.desktop", "notepad.desktop", "extra-editor.desktop"]
        );

        let image_apps = associations.get("image/png").unwrap();
        assert_eq!(image_apps, &vec!["viewer.desktop"]);
    }

    #[test]
    fn test_parse_mimeapps_file_added_associations_only() {
        let mut associations = HashMap::new();
        let content = r#"[Added Associations]
text/html=browser.desktop;editor.desktop;
application/pdf=reader.desktop;"#;

        MimeAssociations::parse_mimeapps_file(&content, &mut associations);

        let html_apps = associations.get("text/html").unwrap();
        assert_eq!(html_apps, &vec!["browser.desktop", "editor.desktop"]);

        let pdf_apps = associations.get("application/pdf").unwrap();
        assert_eq!(pdf_apps, &vec!["reader.desktop"]);
    }

    #[test]
    fn test_parse_mimeapps_file_empty_entries() {
        let mut associations = HashMap::new();
        let content = r#"[Default Applications]
text/plain=editor.desktop;;notepad.desktop;
image/jpeg=;"#;

        MimeAssociations::parse_mimeapps_file(&content, &mut associations);

        let text_apps = associations.get("text/plain").unwrap();
        assert_eq!(text_apps, &vec!["editor.desktop", "notepad.desktop"]);

        // Empty value should result in no entry
        assert!(!associations.contains_key("image/jpeg"));
    }

    #[test]
    fn test_parse_mimeapps_file_with_comments() {
        let mut associations = HashMap::new();
        let content = r#"# This is a comment
[Default Applications]
# Another comment
text/plain=editor.desktop;
# Comment in the middle
image/png=viewer.desktop;"#;

        MimeAssociations::parse_mimeapps_file(&content, &mut associations);

        assert_eq!(
            associations.get("text/plain").unwrap(),
            &vec!["editor.desktop"]
        );
        assert_eq!(
            associations.get("image/png").unwrap(),
            &vec!["viewer.desktop"]
        );
    }

    #[test]
    fn test_get_associations_existing() {
        let mut associations = HashMap::new();
        associations.insert("text/plain".to_string(), vec!["editor.desktop".to_string()]);

        let mime_assoc = MimeAssociations::with_associations(associations);

        let apps = mime_assoc.get_associations("text/plain");
        assert_eq!(apps, vec!["editor.desktop"]);
    }

    #[test]
    fn test_get_associations_non_existing() {
        let associations = HashMap::new();
        let mime_assoc = MimeAssociations::with_associations(associations);

        let apps = mime_assoc.get_associations("application/unknown");
        assert!(apps.is_empty());
    }

    #[test]
    fn test_load_from_multiple_files() {
        // This test would require mocking the file system
        // For now, we'll test the parsing logic thoroughly above
        // In a real scenario, you'd use a test fixture directory
    }
}
