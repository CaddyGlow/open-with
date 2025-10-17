use crate::cache::DesktopCache;
use crate::desktop_parser::DesktopFile;
use crate::mime_associations::MimeAssociations;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationEntry {
    pub name: String,
    pub exec: String,
    pub desktop_file: PathBuf,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub is_xdg: bool,
    pub xdg_priority: i32,
    pub is_default: bool,
    pub action_id: Option<String>,
}

impl ApplicationEntry {
    pub fn from_desktop_entry(
        entry: &crate::desktop_parser::DesktopEntry,
        desktop_file: PathBuf,
    ) -> Self {
        Self {
            name: entry.name.clone(),
            exec: entry.exec.clone(),
            desktop_file,
            comment: entry.comment.clone(),
            icon: entry.icon.clone(),
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: None,
        }
    }

    pub fn from_desktop_action(
        main_entry: &crate::desktop_parser::DesktopEntry,
        action_id: &str,
        action: &crate::desktop_parser::DesktopAction,
        desktop_file: PathBuf,
    ) -> Self {
        Self {
            name: format!("{} - {}", main_entry.name, action.name),
            exec: action.exec.clone(),
            desktop_file,
            comment: Some(format!("Action: {}", action.name)),
            icon: action.icon.clone().or_else(|| main_entry.icon.clone()),
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: Some(action_id.to_string()),
        }
    }

    pub fn with_xdg(mut self, priority: i32, is_default: bool) -> Self {
        self.is_xdg = true;
        self.xdg_priority = priority;
        self.is_default = is_default;
        self
    }

    pub fn as_available(mut self) -> Self {
        self.is_xdg = false;
        self.xdg_priority = -1;
        self.is_default = false;
        self
    }
}

pub struct ApplicationFinder {
    desktop_cache: Box<dyn DesktopCache>,
    mime_associations: MimeAssociations,
}

impl fmt::Debug for ApplicationFinder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationFinder")
            // Desktop cache is a trait object; surface useful summary instead of Debug.
            .field("desktop_cache_len", &self.desktop_cache.len())
            .field("mime_associations", &self.mime_associations)
            .finish()
    }
}

impl ApplicationFinder {
    pub fn new(desktop_cache: Box<dyn DesktopCache>, mime_associations: MimeAssociations) -> Self {
        Self {
            desktop_cache,
            mime_associations,
        }
    }

    pub fn find_for_mime(&self, mime_type: &str, include_actions: bool) -> Vec<ApplicationEntry> {
        let mut applications = Vec::new();
        let mut seen = HashSet::new();

        let xdg_associations = self.mime_associations.get_associations(mime_type);

        // Add XDG associated applications first
        for (priority, desktop_id) in xdg_associations.iter().enumerate() {
            if let Some((path, desktop_file)) = self.find_desktop_file(desktop_id) {
                if seen.insert(desktop_id.clone()) {
                    if let Some(entry) = &desktop_file.main_entry {
                        let priority_i32 = i32::try_from(priority).unwrap_or(i32::MAX);
                        let is_default = priority == 0;

                        let app_entry = ApplicationEntry::from_desktop_entry(entry, path.clone())
                            .with_xdg(priority_i32, is_default);
                        applications.push(app_entry);

                        if include_actions {
                            for (action_id, action) in &desktop_file.actions {
                                let action_app = ApplicationEntry::from_desktop_action(
                                    entry,
                                    action_id,
                                    action,
                                    path.clone(),
                                )
                                .with_xdg(priority_i32, false);
                                applications.push(action_app);
                            }
                        }
                    }
                }
            }
        }

        // Add other applications that support this MIME type
        for (path, desktop_file) in self.desktop_cache.iter() {
            if let Some(entry) = &desktop_file.main_entry {
                if entry.mime_types.contains(&mime_type.to_string()) {
                    let desktop_id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    if seen.insert(desktop_id) {
                        let app = ApplicationEntry::from_desktop_entry(entry, path.clone())
                            .as_available();
                        applications.push(app);

                        if include_actions {
                            for (action_id, action) in &desktop_file.actions {
                                let action_app = ApplicationEntry::from_desktop_action(
                                    entry,
                                    action_id,
                                    action,
                                    path.clone(),
                                )
                                .as_available();
                                applications.push(action_app);
                            }
                        }
                    }
                }
            }
        }

        applications
    }

    pub fn find_desktop_file(&self, desktop_id: &str) -> Option<(&PathBuf, &DesktopFile)> {
        // First try exact filename match
        for (path, desktop_file) in self.desktop_cache.iter() {
            if path.file_name().and_then(|n| n.to_str()) == Some(desktop_id) {
                return Some((path, desktop_file));
            }
        }

        // Then try suffix match
        for (path, desktop_file) in self.desktop_cache.iter() {
            if path.to_string_lossy().ends_with(desktop_id) {
                return Some((path, desktop_file));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_parser::DesktopEntry;
    use std::collections::HashMap;

    fn create_test_desktop_entry(name: &str, mime_types: Vec<&str>) -> DesktopEntry {
        DesktopEntry {
            name: name.to_string(),
            exec: format!("{} %F", name.to_lowercase()),
            comment: Some(format!("Test application {}", name)),
            icon: Some(format!("{}-icon", name.to_lowercase())),
            mime_types: mime_types.iter().map(|s| s.to_string()).collect(),
            no_display: false,
            hidden: false,
            terminal: false,
        }
    }

    fn create_test_desktop_file(entry: DesktopEntry) -> DesktopFile {
        DesktopFile {
            main_entry: Some(entry),
            actions: HashMap::new(),
        }
    }

    fn create_test_application_entry(name: &str) -> ApplicationEntry {
        ApplicationEntry {
            name: name.to_string(),
            exec: format!("{} %F", name.to_lowercase()),
            desktop_file: PathBuf::from(format!(
                "/usr/share/applications/{}.desktop",
                name.to_lowercase()
            )),
            comment: Some(format!("Test application {}", name)),
            icon: Some(format!("{}-icon", name.to_lowercase())),
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: None,
        }
    }

    #[test]
    fn test_new_application_finder() {
        let cache = Box::new(crate::cache::MemoryCache::new());
        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        assert!(finder.desktop_cache.is_empty());
    }

    #[test]
    fn test_find_for_mime_empty_cache() {
        let cache = Box::new(crate::cache::MemoryCache::new());
        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("text/plain", false);
        assert!(apps.is_empty());
    }

    #[test]
    fn test_find_for_mime_with_matching_app() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("TextEditor", vec!["text/plain", "text/html"]);
        let desktop_file = create_test_desktop_file(entry);

        let path = PathBuf::from("/usr/share/applications/texteditor.desktop");
        cache.insert(path.clone(), desktop_file);

        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("text/plain", false);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "TextEditor");
        assert_eq!(apps[0].exec, "texteditor %F");
        assert!(!apps[0].is_xdg);
        assert!(!apps[0].is_default);
        assert_eq!(apps[0].xdg_priority, -1);
    }

    #[test]
    fn test_find_for_mime_with_xdg_associations() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("XDGEditor", vec!["text/plain"]);
        let desktop_file = create_test_desktop_file(entry);

        let path = PathBuf::from("/usr/share/applications/xdgeditor.desktop");
        cache.insert(path.clone(), desktop_file);

        let mut associations_map = HashMap::new();
        associations_map.insert(
            "text/plain".to_string(),
            vec!["xdgeditor.desktop".to_string()],
        );
        let associations = MimeAssociations::with_associations(associations_map);

        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("text/plain", false);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "XDGEditor");
        assert!(apps[0].is_xdg);
        assert!(apps[0].is_default);
        assert_eq!(apps[0].xdg_priority, 0);
    }

    #[test]
    fn test_find_for_mime_with_actions() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("ImageViewer", vec!["image/png"]);

        let mut actions = HashMap::new();
        actions.insert(
            "edit".to_string(),
            crate::desktop_parser::DesktopAction {
                name: "Edit Image".to_string(),
                exec: "imageviewer --edit %F".to_string(),
                icon: Some("edit-icon".to_string()),
            },
        );

        let desktop_file = DesktopFile {
            main_entry: Some(entry),
            actions,
        };

        let path = PathBuf::from("/usr/share/applications/imageviewer.desktop");
        cache.insert(path.clone(), desktop_file);

        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("image/png", true);
        assert_eq!(apps.len(), 2); // Main entry + 1 action

        // Check main entry
        assert_eq!(apps[0].name, "ImageViewer");
        assert!(apps[0].action_id.is_none());

        // Check action
        assert_eq!(apps[1].name, "ImageViewer - Edit Image");
        assert_eq!(apps[1].action_id, Some("edit".to_string()));
        assert_eq!(apps[1].exec, "imageviewer --edit %F");
    }

    #[test]
    fn test_find_for_mime_without_actions() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("ImageViewer", vec!["image/png"]);

        let mut actions = HashMap::new();
        actions.insert(
            "edit".to_string(),
            crate::desktop_parser::DesktopAction {
                name: "Edit Image".to_string(),
                exec: "imageviewer --edit %F".to_string(),
                icon: Some("edit-icon".to_string()),
            },
        );

        let desktop_file = DesktopFile {
            main_entry: Some(entry),
            actions,
        };

        let path = PathBuf::from("/usr/share/applications/imageviewer.desktop");
        cache.insert(path.clone(), desktop_file);

        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("image/png", false);
        assert_eq!(apps.len(), 1); // Only main entry, no actions
        assert_eq!(apps[0].name, "ImageViewer");
        assert!(apps[0].action_id.is_none());
    }

    #[test]
    fn test_find_desktop_file_exact_match() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("TestApp", vec!["text/plain"]);
        let desktop_file = create_test_desktop_file(entry);

        let path = PathBuf::from("/usr/share/applications/testapp.desktop");
        cache.insert(path.clone(), desktop_file);

        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let result = finder.find_desktop_file("testapp.desktop");
        assert!(result.is_some());
        let (found_path, _) = result.unwrap();
        assert_eq!(found_path, &path);
    }

    #[test]
    fn test_find_desktop_file_suffix_match() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("TestApp", vec!["text/plain"]);
        let desktop_file = create_test_desktop_file(entry);

        let path = PathBuf::from("/usr/share/applications/org.example.testapp.desktop");
        cache.insert(path.clone(), desktop_file);

        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let result = finder.find_desktop_file("applications/org.example.testapp.desktop");
        assert!(result.is_some());
        let (found_path, _) = result.unwrap();
        assert_eq!(found_path, &path);
    }

    #[test]
    fn test_find_desktop_file_not_found() {
        let cache = Box::new(crate::cache::MemoryCache::new());
        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let result = finder.find_desktop_file("nonexistent.desktop");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_for_mime_deduplication() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("DuplicateApp", vec!["text/plain"]);
        let desktop_file = create_test_desktop_file(entry);

        let path = PathBuf::from("/usr/share/applications/duplicateapp.desktop");
        cache.insert(path.clone(), desktop_file);

        // Add XDG association for the same app
        let mut associations_map = HashMap::new();
        associations_map.insert(
            "text/plain".to_string(),
            vec!["duplicateapp.desktop".to_string()],
        );
        let associations = MimeAssociations::with_associations(associations_map);

        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("text/plain", false);
        assert_eq!(apps.len(), 1); // Should not be duplicated
        assert_eq!(apps[0].name, "DuplicateApp");
        assert!(apps[0].is_xdg); // Should be marked as XDG since it was found there first
    }

    #[test]
    fn test_find_for_mime_multiple_xdg_priorities() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());

        // Create two apps
        let entry1 = create_test_desktop_entry("FirstApp", vec!["text/plain"]);
        let entry2 = create_test_desktop_entry("SecondApp", vec!["text/plain"]);

        let path1 = PathBuf::from("/usr/share/applications/firstapp.desktop");
        let path2 = PathBuf::from("/usr/share/applications/secondapp.desktop");

        cache.insert(path1.clone(), create_test_desktop_file(entry1));
        cache.insert(path2.clone(), create_test_desktop_file(entry2));

        // Add XDG associations with priorities
        let mut associations_map = HashMap::new();
        associations_map.insert(
            "text/plain".to_string(),
            vec![
                "firstapp.desktop".to_string(),
                "secondapp.desktop".to_string(),
            ],
        );
        let associations = MimeAssociations::with_associations(associations_map);

        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("text/plain", false);
        assert_eq!(apps.len(), 2);

        // First app should be default with priority 0
        let first_app = apps.iter().find(|a| a.name == "FirstApp").unwrap();
        assert!(first_app.is_default);
        assert_eq!(first_app.xdg_priority, 0);

        // Second app should not be default with priority 1
        let second_app = apps.iter().find(|a| a.name == "SecondApp").unwrap();
        assert!(!second_app.is_default);
        assert_eq!(second_app.xdg_priority, 1);
    }

    #[test]
    fn test_find_for_mime_no_matching_mime_type() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());
        let entry = create_test_desktop_entry("TextEditor", vec!["text/plain"]);
        let desktop_file = create_test_desktop_file(entry);

        let path = PathBuf::from("/usr/share/applications/texteditor.desktop");
        cache.insert(path.clone(), desktop_file);

        let associations = MimeAssociations::new();
        let finder = ApplicationFinder::new(cache, associations);

        let apps = finder.find_for_mime("image/png", false);
        assert!(apps.is_empty());
    }

    #[test]
    fn test_application_entry_from_desktop_entry() {
        let entry = create_test_desktop_entry("FromEntry", vec!["text/plain"]);
        let path = PathBuf::from("/usr/share/applications/fromentry.desktop");

        let app = ApplicationEntry::from_desktop_entry(&entry, path.clone());

        assert_eq!(app.name, "FromEntry");
        assert_eq!(app.exec, "fromentry %F");
        assert_eq!(app.desktop_file, path);
        assert_eq!(app.comment, Some("Test application FromEntry".to_string()));
        assert_eq!(app.icon, Some("fromentry-icon".to_string()));
        assert!(!app.is_xdg);
        assert_eq!(app.xdg_priority, -1);
    }

    #[test]
    fn test_application_entry_from_desktop_action() {
        let entry = create_test_desktop_entry("ActionApp", vec!["image/png"]);
        let action = crate::desktop_parser::DesktopAction {
            name: "Edit Image".to_string(),
            exec: "actionapp --edit %F".to_string(),
            icon: Some("edit-icon".to_string()),
        };
        let path = PathBuf::from("/usr/share/applications/actionapp.desktop");

        let app = ApplicationEntry::from_desktop_action(&entry, "edit", &action, path.clone());

        assert_eq!(app.name, "ActionApp - Edit Image");
        assert_eq!(app.exec, "actionapp --edit %F");
        assert_eq!(app.desktop_file, path);
        assert_eq!(app.comment, Some("Action: Edit Image".to_string()));
        assert_eq!(app.icon, Some("edit-icon".to_string()));
        assert_eq!(app.action_id, Some("edit".to_string()));
        assert!(!app.is_xdg);
    }

    #[test]
    fn test_application_entry_with_xdg() {
        let entry = create_test_desktop_entry("XDGApp", vec!["text/plain"]);
        let path = PathBuf::from("/usr/share/applications/xdgapp.desktop");

        let app = ApplicationEntry::from_desktop_entry(&entry, path).with_xdg(2, false);

        assert!(app.is_xdg);
        assert!(!app.is_default);
        assert_eq!(app.xdg_priority, 2);
    }

    #[test]
    fn test_application_entry_with_xdg_default() {
        let entry = create_test_desktop_entry("DefaultApp", vec!["text/plain"]);
        let path = PathBuf::from("/usr/share/applications/defaultapp.desktop");

        let app = ApplicationEntry::from_desktop_entry(&entry, path).with_xdg(0, true);

        assert!(app.is_xdg);
        assert!(app.is_default);
        assert_eq!(app.xdg_priority, 0);
    }

    #[test]
    fn test_application_entry_as_available_resets_flags() {
        let entry = create_test_desktop_entry("ResetApp", vec!["text/plain"]);
        let path = PathBuf::from("/usr/share/applications/resetapp.desktop");

        let app = ApplicationEntry::from_desktop_entry(&entry, path)
            .with_xdg(3, true)
            .as_available();

        assert!(!app.is_xdg);
        assert!(!app.is_default);
        assert_eq!(app.xdg_priority, -1);
    }
}
