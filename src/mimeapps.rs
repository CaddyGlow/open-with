use anyhow::{Context, Result};
use itertools::Itertools;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use wildmatch::WildMatch;

const DEFAULT_SECTION: &str = "[Default Applications]";
const ADDED_SECTION: &str = "[Added Associations]";

/// Represents the user's `mimeapps.list` associations.
#[derive(Debug, Default, Clone)]
pub struct MimeApps {
    default_apps: BTreeMap<String, DesktopList>,
    added_associations: BTreeMap<String, DesktopList>,
}

impl MimeApps {
    /// Load `mimeapps.list` from disk, returning an empty structure when the file does not exist.
    pub fn load_from_disk(path: Option<PathBuf>) -> Result<Self> {
        let path = path.unwrap_or_else(Self::default_path);

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        Ok(Self::parse(&contents))
    }

    /// Write the current associations back to disk.
    pub fn save_to_disk(&self, path: Option<PathBuf>) -> Result<()> {
        let path = path.unwrap_or_else(Self::default_path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let mut file = fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        self.write(&mut file)?;
        Ok(())
    }

    /// Parse associations from a raw INI string.
    pub fn parse(contents: &str) -> Self {
        let mut current_section = None;
        let mut default_apps = BTreeMap::new();
        let mut added_associations = BTreeMap::new();

        for line in contents.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(section) = parse_section_header(line) {
                current_section = Some(section);
                continue;
            }

            if let Some((mime, handlers)) = parse_assignment(line) {
                let target_map: &mut BTreeMap<String, DesktopList> = match current_section {
                    Some(Section::DefaultApplications) => &mut default_apps,
                    Some(Section::AddedAssociations) => &mut added_associations,
                    _ => continue,
                };

                if !handlers.is_empty() {
                    let list = target_map.entry(mime.to_string()).or_default();
                    list.extend(handlers.into_iter().map(str::to_owned));
                    list.dedup();
                }
            }
        }

        Self {
            default_apps,
            added_associations,
        }
    }

    /// Serialize associations into an INI-style writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        fn write_section<W: Write>(
            writer: &mut W,
            name: &str,
            entries: &BTreeMap<String, DesktopList>,
        ) -> Result<()> {
            if entries.is_empty() {
                return Ok(());
            }

            writeln!(writer, "{name}")?;
            for (mime, handlers) in entries {
                if handlers.is_empty() {
                    continue;
                }
                let joined = handlers.iter().map(|h| h.as_str()).join(";");
                writeln!(writer, "{mime}={joined};")?;
            }
            writeln!(writer)?;
            Ok(())
        }

        write_section(writer, DEFAULT_SECTION, &self.default_apps)?;
        write_section(writer, ADDED_SECTION, &self.added_associations)?;
        Ok(())
    }

    /// Replace the list of handlers for the provided mimetype pattern.
    pub fn set_handler(&mut self, pattern: &str, handlers: Vec<String>, expand_wildcards: bool) {
        self.apply_to_mimes(pattern, expand_wildcards, |entry| {
            entry.clear();
            entry.extend(handlers.iter().cloned());
            entry.dedup();
        });
    }

    /// Append a handler to the mimetype pattern if it is not already present.
    pub fn add_handler(&mut self, pattern: &str, handler: String, expand_wildcards: bool) {
        self.apply_to_mimes(pattern, expand_wildcards, |entry| {
            if !entry.contains(&handler) {
                entry.push_back(handler.clone());
            }
        });
    }

    /// Remove a handler from the mimetype pattern. When `handler` is `None`, the entire entry is removed.
    pub fn remove_handler(&mut self, pattern: &str, handler: Option<&str>, expand_wildcards: bool) {
        self.apply_to_mimes(pattern, expand_wildcards, |entry| {
            if let Some(target) = handler {
                entry.retain(|h| h != target);
            } else {
                entry.clear();
            }
        });

        self.default_apps.retain(|_, list| !list.is_empty());
    }

    /// Return the handlers configured for the given MIME type.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn handlers_for(&self, mime: &str) -> Option<&DesktopList> {
        self.default_apps.get(mime)
    }

    /// Expose the default applications map.
    pub fn default_apps(&self) -> &BTreeMap<String, DesktopList> {
        &self.default_apps
    }

    /// Expose Added Associations for consumers.
    pub fn added_associations(&self) -> &BTreeMap<String, DesktopList> {
        &self.added_associations
    }

    fn apply_to_mimes<F>(&mut self, pattern: &str, expand_wildcards: bool, mut f: F)
    where
        F: FnMut(&mut DesktopList),
    {
        let targets = self.resolve_targets(pattern, expand_wildcards);

        if targets.is_empty() && !expand_wildcards {
            let entry = self.default_apps.entry(pattern.to_string()).or_default();
            f(entry);
            return;
        }

        for mime in targets {
            let entry = self.default_apps.entry(mime).or_default();
            f(entry);
        }
    }

    fn resolve_targets(&self, pattern: &str, expand_wildcards: bool) -> Vec<String> {
        if !expand_wildcards || !pattern.contains('*') {
            return vec![pattern.to_string()];
        }

        let matcher = WildMatch::new(pattern);
        self.default_apps
            .keys()
            .chain(self.added_associations.keys())
            .filter(|mime| matcher.matches(mime))
            .cloned()
            .collect()
    }

    fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mimeapps.list")
    }
}

/// Wrapper around the handler queue captured for a MIME type.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DesktopList(VecDeque<String>);

impl DesktopList {
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.0.extend(iter);
    }

    pub fn push_back(&mut self, handler: String) {
        self.0.push_back(handler);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&String) -> bool,
    {
        self.0.retain(|h| f(h));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, handler: &str) -> bool {
        self.0.iter().any(|h| h == handler)
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }

    pub fn dedup(&mut self) {
        let mut seen = Vec::new();
        self.0.retain(|handler| {
            if seen.contains(handler) {
                false
            } else {
                seen.push(handler.clone());
                true
            }
        });
    }
}

fn parse_section_header(line: &str) -> Option<Section> {
    match line {
        DEFAULT_SECTION => Some(Section::DefaultApplications),
        ADDED_SECTION => Some(Section::AddedAssociations),
        _ => None,
    }
}

fn parse_assignment(line: &str) -> Option<(&str, Vec<&str>)> {
    let (key, value) = line.split_once('=')?;
    let handlers = value
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    Some((key.trim(), handlers))
}

#[derive(Debug, Clone, Copy)]
enum Section {
    DefaultApplications,
    AddedAssociations,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_and_write_round_trip() {
        let input = r#"[Default Applications]
text/plain=helix.desktop;vim.desktop;

[Added Associations]
text/plain=code.desktop;
"#;

        let apps = MimeApps::parse(input);
        let mut output = Vec::new();
        apps.write(&mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("text/plain=helix.desktop;vim.desktop;"));
        assert!(output_str.contains("text/plain=code.desktop;"));
    }

    #[test]
    fn load_and_save_disk() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("mimeapps.list");
        fs::write(&path, "[Default Applications]\ntext/plain=helix.desktop;\n").unwrap();

        let mut apps = MimeApps::load_from_disk(Some(path.clone())).unwrap();
        apps.add_handler("text/plain", "code.desktop".to_string(), false);
        apps.save_to_disk(Some(path.clone())).unwrap();

        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains("code.desktop"));
    }

    #[test]
    fn wildcard_resolution_without_expand_keeps_pattern() {
        let mut apps = MimeApps::default();
        apps.set_handler("text/*", vec!["helix.desktop".into()], false);

        assert!(apps.handlers_for("text/*").is_some());
    }

    #[test]
    fn wildcard_resolution_with_expand_matches_existing_keys() {
        let mut apps = MimeApps::default();
        apps.set_handler("text/plain", vec!["helix.desktop".into()], false);
        apps.add_handler("text/*", "code.desktop".into(), true);

        let handlers = apps.handlers_for("text/plain").unwrap();
        assert!(handlers.contains("code.desktop"));
    }

    #[test]
    fn remove_handler_cleans_up_entries() {
        let mut apps = MimeApps::default();
        apps.set_handler("text/plain", vec!["helix.desktop".into()], false);
        apps.remove_handler("text/plain", Some("helix.desktop"), false);

        assert!(apps.handlers_for("text/plain").is_none());
    }
}
