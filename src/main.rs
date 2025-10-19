use anyhow::Result;
use clap::{CommandFactory, Parser};

mod application_finder;
mod cache;
mod cli;
mod commands;
mod config;
mod desktop_parser;
mod executor;
mod fuzzy_finder;
mod mime_associations;
mod mime_pattern;
mod mimeapps;
mod open_it;
mod regex_handlers;
mod selector;
mod target;
mod template;
mod xdg;

#[cfg(test)]
pub mod test_support;

// Build info module
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

use cli::Cli;

fn main() -> Result<()> {
    clap_complete::CompleteEnv::with_factory(|| Cli::command().name("openit"))
        .completer("openit")
        .complete();

    let cli = Cli::parse();
    commands::dispatch(cli.command)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_finder::{ApplicationEntry, ApplicationFinder};
    use crate::cache::DesktopCache;
    use crate::desktop_parser::DesktopFile;
    use crate::executor::ApplicationExecutor;
    use crate::mime_associations::MimeAssociations;
    use crate::target::LaunchTarget;
    use crate::test_support::create_test_desktop_file;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_application_entry_serialization() {
        let app = ApplicationEntry {
            name: "Test App".to_string(),
            exec: "test-app %F".to_string(),
            desktop_file: PathBuf::from("/usr/share/applications/test.desktop"),
            comment: Some("Test application".to_string()),
            icon: Some("test-icon".to_string()),
            is_xdg: true,
            xdg_priority: 0,
            is_default: true,
            action_id: None,
            requires_terminal: false,
            is_terminal_emulator: false,
        };

        let json = serde_json::to_string(&app).unwrap();
        let deserialized: ApplicationEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(app.name, deserialized.name);
        assert_eq!(app.exec, deserialized.exec);
        assert_eq!(app.is_default, deserialized.is_default);
    }

    #[test]
    fn test_clean_exec_command() {
        let target = LaunchTarget::File(PathBuf::from("/tmp/test.txt"));
        let test_cases = vec![
            ("app %f", "app"),
            ("app %F %u", "app"),
            ("app %%", "app"),
            ("app %i %c %k", "app"),
            ("  app %f  ", "app"),
        ];

        for (input, expected_command) in test_cases {
            let command = ApplicationExecutor::prepare_command(input, &target)
                .unwrap_or_else(|e| panic!("Command preparation failed for {input}: {e}"));
            assert_eq!(
                command.first().unwrap(),
                expected_command,
                "Failed for input: {input}"
            );

            if input.contains("%%") {
                assert!(
                    command.iter().any(|arg| arg == "%"),
                    "Expected literal % in args for input: {input}"
                );
            }
        }
    }

    #[test]
    fn test_find_desktop_file_exact_match() {
        let temp_dir = TempDir::new().unwrap();
        let desktop_content = r"[Desktop Entry]
Name=Test
Exec=test";

        let file_path = create_test_desktop_file(temp_dir.path(), "test.desktop", desktop_content);

        let mut cache = Box::new(cache::MemoryCache::new());
        let desktop_file = DesktopFile::parse(&file_path).unwrap();
        cache.insert(file_path.clone(), desktop_file);

        let mime_associations = MimeAssociations::default();
        let application_finder = ApplicationFinder::new(cache, mime_associations);

        let result = application_finder.find_desktop_file("test.desktop");
        assert!(result.is_some());
    }

    #[test]
    fn test_build_info_constants() {
        // Test that build info constants are available
        assert!(!built_info::PKG_VERSION.is_empty());
        // assert!(!built_info::BUILT_TIME_UTC.is_empty());
        assert!(!built_info::TARGET.is_empty());
        assert!(!built_info::RUSTC_VERSION.is_empty());
    }

    #[test]
    fn test_get_applications_for_mime_with_actions() {
        let temp_dir = TempDir::new().unwrap();
        let desktop_content = r"[Desktop Entry]
Name=Test App
Exec=testapp %F
MimeType=text/plain;
Actions=edit;print;

[Desktop Action edit]
Name=Edit
Exec=testapp --edit %F

[Desktop Action print]
Name=Print
Exec=testapp --print %F";

        let file_path = create_test_desktop_file(temp_dir.path(), "test.desktop", desktop_content);

        let mut cache = Box::new(cache::MemoryCache::new());
        let desktop_file = DesktopFile::parse(&file_path).unwrap();
        cache.insert(file_path.clone(), desktop_file);

        let mime_associations = MimeAssociations::default();
        let application_finder = ApplicationFinder::new(cache, mime_associations);

        let apps = application_finder.find_for_mime("text/plain", true);

        // Should have main entry + 2 actions = 3 total
        assert_eq!(apps.len(), 3);

        // Check main entry
        assert_eq!(apps[0].name, "Test App");
        assert!(apps[0].action_id.is_none());

        // Check actions - order might vary, so check both possibilities
        let action_names: Vec<&str> = apps[1..].iter().map(|a| a.name.as_str()).collect();
        assert!(action_names.contains(&"Test App - Edit"));
        assert!(action_names.contains(&"Test App - Print"));

        // Find the edit action
        let edit_action = apps
            .iter()
            .find(|a| a.action_id == Some("edit".to_string()))
            .unwrap();
        assert_eq!(edit_action.name, "Test App - Edit");

        // Find the print action
        let print_action = apps
            .iter()
            .find(|a| a.action_id == Some("print".to_string()))
            .unwrap();
        assert_eq!(print_action.name, "Test App - Print");
    }

    #[test]
    fn test_get_applications_for_mime_with_xdg_associations() {
        let temp_dir = TempDir::new().unwrap();
        let desktop_content = r"[Desktop Entry]
Name=XDG App
Exec=xdgapp %F
MimeType=text/plain;";

        let file_path = create_test_desktop_file(temp_dir.path(), "xdg.desktop", desktop_content);

        let mut cache = Box::new(cache::MemoryCache::new());
        let desktop_file = DesktopFile::parse(&file_path).unwrap();
        cache.insert(file_path.clone(), desktop_file);

        // Create mime associations with xdg.desktop as default
        let mut associations = HashMap::new();
        associations.insert("text/plain".to_string(), vec!["xdg.desktop".to_string()]);

        let mime_associations = MimeAssociations::with_associations(associations);
        let application_finder = ApplicationFinder::new(cache, mime_associations);

        let apps = application_finder.find_for_mime("text/plain", false);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "XDG App");
        assert!(apps[0].is_xdg);
        assert_eq!(apps[0].xdg_priority, 0);
        assert!(apps[0].is_default);
    }

    #[test]
    fn test_find_desktop_file_with_path_suffix() {
        let temp_dir = TempDir::new().unwrap();
        let desktop_content = r"[Desktop Entry]
Name=Test
Exec=test";

        // Create a desktop file in a subdirectory
        let subdir = temp_dir.path().join("applications");
        fs::create_dir_all(&subdir).unwrap();
        let file_path =
            create_test_desktop_file(&subdir, "org.example.test.desktop", desktop_content);

        let mut cache = Box::new(cache::MemoryCache::new());
        let desktop_file = DesktopFile::parse(&file_path).unwrap();
        cache.insert(file_path.clone(), desktop_file);

        let mime_associations = MimeAssociations::default();
        let application_finder = ApplicationFinder::new(cache, mime_associations);

        // Should find by suffix match
        let result = application_finder.find_desktop_file("applications/org.example.test.desktop");
        assert!(result.is_some());
    }

    #[test]
    fn test_execute_application_command_parsing() {
        // Test the exec command cleaning logic
        let test_cases = vec![
            ("app %f %u", "app"),
            ("app %F %U", "app"),
            ("app %i %c %k", "app"),
            ("app %%", "app %"),
            ("  app %f  ", "app"),
        ];

        for (input, expected) in test_cases {
            let app = ApplicationEntry {
                name: "Test".to_string(),
                exec: input.to_string(),
                desktop_file: PathBuf::from("test.desktop"),
                comment: None,
                icon: None,
                is_xdg: false,
                xdg_priority: -1,
                is_default: false,
                action_id: None,
                requires_terminal: false,
                is_terminal_emulator: false,
            };

            // Extract the cleaning logic to test it
            let clean_exec = app
                .exec
                .replace("%u", "")
                .replace("%U", "")
                .replace("%f", "")
                .replace("%F", "")
                .replace("%i", "")
                .replace("%c", "")
                .replace("%k", "")
                .replace("%%", "%")
                .trim()
                .to_string();

            assert_eq!(clean_exec, expected, "Failed for input: {input}");
        }
    }

    #[test]
    fn test_execute_application_empty_exec() {
        let app = ApplicationEntry {
            name: "Empty Exec".to_string(),
            exec: "   %f %F   ".to_string(), // Will become empty after cleaning
            desktop_file: PathBuf::from("test.desktop"),
            comment: None,
            icon: None,
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: None,
            requires_terminal: false,
            is_terminal_emulator: false,
        };

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        let target = LaunchTarget::File(test_file);

        let executor = ApplicationExecutor::new();
        let result = executor.execute(&app, &target, None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty exec command");
    }

    #[test]
    fn test_get_applications_with_duplicate_desktop_ids() {
        let temp_dir = TempDir::new().unwrap();

        // Create two desktop files with same content
        let desktop_content = r"[Desktop Entry]
Name=Duplicate App
Exec=app %F
MimeType=text/plain;";

        let file1 = temp_dir.path().join("app1.desktop");
        let file2 = temp_dir.path().join("app2.desktop");
        fs::write(&file1, desktop_content).unwrap();
        fs::write(&file2, desktop_content).unwrap();

        let mut cache = Box::new(cache::MemoryCache::new());
        cache.insert(file1.clone(), DesktopFile::parse(&file1).unwrap());
        cache.insert(file2.clone(), DesktopFile::parse(&file2).unwrap());

        // Create associations pointing to app1.desktop
        let mut associations = HashMap::new();
        associations.insert("text/plain".to_string(), vec!["app1.desktop".to_string()]);

        let mime_associations = MimeAssociations::with_associations(associations);
        let application_finder = ApplicationFinder::new(cache, mime_associations);

        let apps = application_finder.find_for_mime("text/plain", false);

        // Should have both apps, but app1 should be marked as XDG associated
        assert_eq!(apps.len(), 2);

        let xdg_app = apps.iter().find(|a| a.is_xdg).unwrap();
        assert!(xdg_app.desktop_file.ends_with("app1.desktop"));
    }
}
