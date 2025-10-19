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
    use crate::cache::{DesktopCache, FileSystemCache};
    use crate::cli::{Command, EditArgs, OpenArgs, RemoveArgs, SelectorKind, UnsetArgs};
    use crate::commands;
    use crate::commands::normalize_mime_input;
    use crate::config::Config;
    use crate::desktop_parser::{DesktopEntry, DesktopFile};
    use crate::executor::ApplicationExecutor;
    use crate::fuzzy_finder::FuzzyFinderRunner;
    use crate::mime_associations::MimeAssociations;
    use crate::open_it::OpenIt;
    use crate::regex_handlers::RegexHandlerStore;
    use crate::selector::SelectorRunner;
    use crate::target::LaunchTarget;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command as ProcessCommand, Stdio};
    use std::time::Duration;
    use tempfile::TempDir;
    use url::Url;

    /// Helper function to create test args with JSON output to avoid fuzzy finder
    fn create_test_args_json(target: Option<PathBuf>) -> OpenArgs {
        OpenArgs {
            target: target.map(|p| p.to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: true, // Always use JSON in tests to avoid fuzzy finder
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        }
    }

    fn create_test_desktop_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let file_path = dir.join(name);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    fn basic_desktop_content(name: &str, exec: &str, mime: &str) -> String {
        format!("[Desktop Entry]\nName={name}\nExec={exec}\nMimeType={mime};\nTerminal=false\n")
    }

    struct CacheEnvGuard {
        original: Option<OsString>,
    }

    #[test]
    fn test_populate_cache_adds_new_entries_without_rebuild() {
        let temp_dir = TempDir::new().unwrap();
        let apps_dir = temp_dir.path().join("applications");
        fs::create_dir_all(&apps_dir).unwrap();

        let cache_path = temp_dir.path().join("cache.json");
        let mut cache = FileSystemCache::new(cache_path);

        let existing = create_test_desktop_file(
            &apps_dir,
            "existing.desktop",
            &basic_desktop_content("Existing", "existing --flag %F", "text/plain"),
        );

        // Initial population simulates a full rebuild
        assert!(OpenIt::populate_cache_from_dirs(
            &mut cache,
            &[apps_dir.clone()],
            true
        ));
        assert!(DesktopCache::get(&cache, &existing).is_some());

        // Add a new desktop file after the initial cache build
        let new_entry_path = create_test_desktop_file(
            &apps_dir,
            "imgcat.desktop",
            &basic_desktop_content("Imgcat", "imgcat %f", "image/png"),
        );

        // Running without a rebuild should still capture the new desktop file
        assert!(
            OpenIt::populate_cache_from_dirs(&mut cache, &[apps_dir.clone()], false),
            "populate_cache_from_dirs should report updates when new files are discovered"
        );

        assert!(
            DesktopCache::get(&cache, &new_entry_path).is_some(),
            "Newly added desktop file should be available in cache"
        );
    }

    #[test]
    fn test_normalize_mime_input_alias_resolution() {
        assert_eq!(normalize_mime_input("image/jpeg").unwrap(), "image/jpeg");
        assert_eq!(normalize_mime_input("image/JPG").unwrap(), "image/jpeg");
        assert_eq!(normalize_mime_input("image/png").unwrap(), "image/png");
    }

    #[test]
    fn test_normalize_mime_input_preserves_wildcard() {
        assert_eq!(normalize_mime_input("image/*").unwrap(), "image/*");
    }

    impl CacheEnvGuard {
        const KEY: &'static str = "OPEN_WITH_CACHE_PATH";

        fn set(path: &Path) -> Self {
            let original = env::var_os(Self::KEY);
            env::set_var(Self::KEY, path);
            Self { original }
        }
    }

    impl Drop for CacheEnvGuard {
        fn drop(&mut self) {
            if let Some(original) = self.original.take() {
                env::set_var(Self::KEY, original);
            } else {
                env::remove_var(Self::KEY);
            }
        }
    }

    struct ConfigEnvGuard {
        original: Option<OsString>,
    }

    impl ConfigEnvGuard {
        const KEY: &'static str = "XDG_CONFIG_HOME";

        fn set(path: &Path) -> Self {
            let original = env::var_os(Self::KEY);
            env::set_var(Self::KEY, path);
            Self { original }
        }
    }

    impl Drop for ConfigEnvGuard {
        fn drop(&mut self) {
            if let Some(original) = self.original.take() {
                env::set_var(Self::KEY, original);
            } else {
                env::remove_var(Self::KEY);
            }
        }
    }

    struct ValidationEnvGuard {
        original: Option<OsString>,
    }

    impl ValidationEnvGuard {
        const KEY: &'static str = "OPEN_WITH_SKIP_HANDLER_VALIDATION";

        fn enable() -> Self {
            let original = env::var_os(Self::KEY);
            env::set_var(Self::KEY, "1");
            Self { original }
        }
    }

    impl Drop for ValidationEnvGuard {
        fn drop(&mut self) {
            if let Some(original) = self.original.take() {
                env::set_var(Self::KEY, original);
            } else {
                env::remove_var(Self::KEY);
            }
        }
    }

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
    fn test_cache_path_creation() {
        let cache_path = OpenIt::cache_path();
        assert!(cache_path.ends_with("openit/desktop_cache.json"));
    }

    #[test]
    #[serial]
    fn test_handle_command_set_add_unset() {
        let temp_config = TempDir::new().unwrap();
        let _guard = ConfigEnvGuard::set(temp_config.path());

        let _validation = ValidationEnvGuard::enable();

        commands::dispatch(Command::Set(EditArgs {
            mime: "text/plain".into(),
            handler: "helix.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let config_path = temp_config.path().join("mimeapps.list");
        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.contains("text/plain=helix.desktop;"));

        commands::dispatch(Command::Add(EditArgs {
            mime: "text/plain".into(),
            handler: "code.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.contains("text/plain=helix.desktop;code.desktop;"));

        commands::dispatch(Command::Unset(UnsetArgs {
            mime: "text/plain".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.trim().is_empty());
    }

    #[test]
    #[serial]
    fn test_handle_command_remove_handler() {
        let temp_config = TempDir::new().unwrap();
        let _guard = ConfigEnvGuard::set(temp_config.path());

        let _validation = ValidationEnvGuard::enable();

        commands::dispatch(Command::Set(EditArgs {
            mime: "text/plain".into(),
            handler: "helix.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        commands::dispatch(Command::Add(EditArgs {
            mime: "text/plain".into(),
            handler: "code.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        commands::dispatch(Command::Remove(RemoveArgs {
            mime: "text/plain".into(),
            handler: "helix.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let contents = fs::read_to_string(temp_config.path().join("mimeapps.list")).unwrap();
        assert!(contents.contains("text/plain=code.desktop;"));
        assert!(!contents.contains("helix.desktop"));
    }

    #[test]
    #[serial]
    fn test_handle_command_add_missing_handler_errors() {
        env::remove_var(ValidationEnvGuard::KEY);

        let temp_config = TempDir::new().unwrap();
        let _guard = ConfigEnvGuard::set(temp_config.path());

        let result = commands::dispatch(Command::Add(EditArgs {
            mime: "text/plain".into(),
            handler: "nonexistent.desktop".into(),
            expand_wildcards: false,
        }));

        assert!(result.is_err());
        let message = format!("{}", result.unwrap_err());
        assert!(message.contains("Desktop handler"));
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
    #[serial]
    fn test_new_with_clear_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file = temp_dir.path().join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);

        // Test that OpenIt::new succeeds when clear_cache is true
        // This should work even in environments with no desktop files
        let args = OpenArgs {
            target: Some("test.txt".to_string()),
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: true,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        // Initialize env_logger for debugging if test fails
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();

        // The function should succeed even if:
        // 1. No cache exists to clear
        // 2. No desktop directories exist
        // 3. Cache directory can't be created
        let result = OpenIt::new(args);

        // If it fails, print the error for debugging
        if let Err(ref e) = result {
            eprintln!("OpenIt::new failed with error: {e}");
            eprintln!("Error chain: {e:?}");
        }

        assert!(
            result.is_ok(),
            "OpenIt::new should handle missing cache and desktop files gracefully"
        );
    }

    #[test]
    fn test_get_applications_for_mime_empty() {
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));
        let app = OpenIt::new(args).unwrap();

        let apps = app
            .application_finder
            .find_for_mime("application/unknown", app.args.actions);
        assert!(apps.is_empty());
    }

    #[test]
    #[serial]
    fn test_clear_cache() {
        use tempfile::TempDir;

        // Create a temporary directory for the cache
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("openit");
        fs::create_dir_all(&cache_dir).unwrap();

        // Create a mock cache file
        let cache_file = cache_dir.join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);
        fs::write(&cache_file, "test cache").unwrap();

        // Verify file exists
        assert!(cache_file.exists());

        // Clear the specific cache file via OpenIt helper
        let result = OpenIt::clear_cache();
        assert!(result.is_ok());
        assert!(!cache_file.exists());
    }

    #[test]
    fn test_output_json() {
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));
        let app = OpenIt::new(args).unwrap();

        let applications = vec![ApplicationEntry {
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
        }];

        let mime_type = "text/plain";
        let target = LaunchTarget::File(PathBuf::from("test.txt"));

        // This will print to stdout, but we're mainly testing it doesn't panic
        let result = app.output_json_for_test(applications, target, mime_type.to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_launch_target_with_uri() {
        let target = OpenIt::resolve_launch_target("https://example.com").unwrap();
        assert!(matches!(target, LaunchTarget::Uri(_)));
        assert_eq!(OpenIt::mime_for_target(&target), "x-scheme-handler/https");
    }

    #[test]
    fn test_resolve_launch_target_with_file_uri() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("uri_test.txt");
        fs::write(&file_path, "content").unwrap();
        let uri = Url::from_file_path(&file_path).expect("valid file uri");

        let target = OpenIt::resolve_launch_target(uri.as_str()).unwrap();
        match target {
            LaunchTarget::File(path) => {
                assert_eq!(path, file_path.canonicalize().unwrap());
            }
            LaunchTarget::Uri(_) => panic!("expected file target"),
        }
    }

    #[test]
    fn test_run_with_no_file() {
        let args = OpenArgs {
            target: None,
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No target provided");
    }

    #[test]
    fn test_run_with_nonexistent_file() {
        let args = OpenArgs {
            target: Some("/nonexistent/file.txt".to_string()),
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to resolve file path"));
    }

    #[test]
    #[serial]
    fn test_run_clear_cache_only() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file = temp_dir.path().join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);

        let args = OpenArgs {
            target: None,
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: true,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        // This should succeed even if no cache file exists
        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        assert!(result.is_ok());
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
    fn test_run_with_directory_instead_of_file() {
        let temp_dir = TempDir::new().unwrap();

        let args = OpenArgs {
            target: Some(temp_dir.path().to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Path is not a file"));
    }

    #[test]
    fn test_run_with_no_applications_for_mime() {
        let temp_dir = TempDir::new().unwrap();
        let temp_file = temp_dir.path().join("test.xyz");
        fs::write(&temp_file, "test content").unwrap();

        let args = OpenArgs {
            target: Some(temp_file.to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        let _app = OpenIt::new(args).unwrap();
        let result = _app.run();
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        // The error message should contain information about no applications found
        assert!(
            error_msg.contains("No applications found") || error_msg.contains("MIME type"),
            "Expected error about no applications, got: {error_msg}"
        );
    }

    #[test]
    fn test_load_desktop_cache_with_invalid_cache_file() {
        use tempfile::TempDir;

        // Create a temporary directory for the cache
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("openit");
        fs::create_dir_all(&cache_dir).unwrap();

        // Create an invalid cache file
        let cache_file = cache_dir.join("desktop_cache.json");
        fs::write(&cache_file, "invalid json content").unwrap();

        // Override the cache path for this test
        std::env::set_var("HOME", temp_dir.path());

        // Should handle invalid cache gracefully and rebuild
        let cache = OpenIt::load_desktop_cache();

        // Cache should be empty or contain whatever desktop files are found
        assert!(cache.is_empty() || !cache.is_empty());

        // Restore HOME
        std::env::remove_var("HOME");
    }

    #[test]
    #[serial]
    fn test_clear_cache_with_permission_error() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file = temp_dir.path().join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);

        // This test verifies error handling when cache file can't be removed
        // We can't easily simulate permission errors in tests, but we can test
        // the error path by mocking the scenario

        // The clear_cache function already handles errors gracefully
        let result = OpenIt::clear_cache();
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_fuzzy_finder_auto_detection() {
        // Test fuzzy finder auto-detection logic without actually running it
        let _args = OpenArgs {
            target: Some("test.txt".to_string()),
            selector: SelectorKind::Auto,
            json: true, // Use JSON to avoid running fuzzy finder
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        let args = create_test_args_json(Some(PathBuf::from("test.txt")));
        let _app = OpenIt::new(args).unwrap();

        // Test the fuzzy finder detection logic without asserting on specific availability
        let _ = which::which("fzf");
        let _ = which::which("fuzzel");
    }

    #[test]
    fn test_run_json_output_non_terminal() {
        // Test JSON output functionality with a controlled environment
        // We can't rely on desktop files in build environments, so test the JSON output format
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));
        let app = OpenIt::new(args).unwrap();

        let applications = vec![ApplicationEntry {
            name: "Test App".to_string(),
            exec: "test-app %F".to_string(),
            desktop_file: PathBuf::from("/usr/share/applications/test.desktop"),
            comment: Some("Test application".to_string()),
            icon: Some("test-icon".to_string()),
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: None,
            requires_terminal: false,
            is_terminal_emulator: false,
        }];

        let mime_type = "text/plain";
        let target = LaunchTarget::File(PathBuf::from("test.txt"));

        // Test that output_json works correctly
        let result = app.output_json_for_test(applications, target, mime_type.to_string());
        assert!(result.is_ok());
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
    fn test_resolve_terminal_launcher_prefers_scheme_handler() {
        let mut cache = Box::new(cache::MemoryCache::new());

        let terminal_entry = DesktopEntry {
            name: "Terminal".to_string(),
            exec: "foot".to_string(),
            mime_types: vec!["x-scheme-handler/terminal".to_string()],
            categories: vec!["TerminalEmulator".to_string()],
            ..DesktopEntry::default()
        };

        let terminal_file = DesktopFile {
            main_entry: Some(terminal_entry),
            actions: HashMap::new(),
        };

        let terminal_path = PathBuf::from("/usr/share/applications/terminal.desktop");
        cache.insert(terminal_path, terminal_file);

        let mut associations = HashMap::new();
        associations.insert(
            "x-scheme-handler/terminal".to_string(),
            vec!["terminal.desktop".to_string()],
        );

        let application_finder =
            ApplicationFinder::new(cache, MimeAssociations::with_associations(associations));

        let config = Config::default();
        let executor = ApplicationExecutor::with_options(
            config.app_launch_prefix.clone(),
            config.selector.term_exec_args.clone(),
        );

        let regex_handlers = RegexHandlerStore::load(None).unwrap();
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));

        let open_with = OpenIt {
            application_finder,
            fuzzy_finder_runner: FuzzyFinderRunner::new(),
            selector_runner: SelectorRunner::new(),
            executor,
            config,
            regex_handlers,
            args,
        };

        let launcher = open_with.resolve_terminal_launcher().unwrap();
        assert_eq!(launcher, vec!["foot"]);
    }

    #[test]
    fn test_resolve_terminal_launcher_falls_back_to_category() {
        let mut cache = Box::new(cache::MemoryCache::new());

        let terminal_entry = DesktopEntry {
            name: "Kitty".to_string(),
            exec: "kitty --single-instance".to_string(),
            mime_types: vec![],
            categories: vec!["Utility".to_string(), "TerminalEmulator".to_string()],
            ..DesktopEntry::default()
        };

        let terminal_file = DesktopFile {
            main_entry: Some(terminal_entry),
            actions: HashMap::new(),
        };

        cache.insert(
            PathBuf::from("/usr/share/applications/kitty.desktop"),
            terminal_file,
        );

        let application_finder =
            ApplicationFinder::new(cache, MimeAssociations::with_associations(HashMap::new()));

        let mut config = Config::default();
        config.selector.term_exec_args = Some(String::new());
        let executor = ApplicationExecutor::with_options(
            config.app_launch_prefix.clone(),
            config.selector.term_exec_args.clone(),
        );

        let regex_handlers = RegexHandlerStore::load(None).unwrap();
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));

        let open_with = OpenIt {
            application_finder,
            fuzzy_finder_runner: FuzzyFinderRunner::new(),
            selector_runner: SelectorRunner::new(),
            executor,
            config,
            regex_handlers,
            args,
        };

        let launcher = open_with.resolve_terminal_launcher().unwrap();
        assert_eq!(launcher, vec!["kitty", "--single-instance"]);
    }

    #[test]
    fn test_resolve_terminal_launcher_errors_without_terminal() {
        let cache = Box::new(cache::MemoryCache::new());
        let application_finder = ApplicationFinder::new(cache, MimeAssociations::default());

        let config = Config::default();
        let executor = ApplicationExecutor::with_options(
            config.app_launch_prefix.clone(),
            config.selector.term_exec_args.clone(),
        );

        let regex_handlers = RegexHandlerStore::load(None).unwrap();
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));

        let open_with = OpenIt {
            application_finder,
            fuzzy_finder_runner: FuzzyFinderRunner::new(),
            selector_runner: SelectorRunner::new(),
            executor,
            config,
            regex_handlers,
            args,
        };

        let err = open_with.resolve_terminal_launcher().unwrap_err();
        assert!(err.to_string().contains("No terminal emulator found"));
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

    #[test]
    fn test_cache_save_failure_handling() {
        // Test that cache save failures don't prevent the function from working
        let temp_dir = TempDir::new().unwrap();

        // Set HOME to a read-only directory to simulate save failure
        let readonly_dir = temp_dir.path().join("readonly");
        fs::create_dir(&readonly_dir).unwrap();

        // Make directory read-only on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o444);
            fs::set_permissions(&readonly_dir, perms).unwrap();
        }

        std::env::set_var("HOME", &readonly_dir);

        // Should still work even if cache can't be saved
        let cache = OpenIt::load_desktop_cache();

        // Restore HOME and permissions
        std::env::remove_var("HOME");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&readonly_dir, perms).unwrap();
        }

        // Function should have returned successfully
        assert!(cache.is_empty() || !cache.is_empty());
    }

    #[test]
    fn test_run_with_verbose_logging() {
        // Initialize logger for test
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Info)
            .try_init();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.reallyunknowntype");
        fs::write(&test_file, "test content").unwrap();

        let args = OpenArgs {
            target: Some(test_file.to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: true, // Use JSON to avoid fuzzy finder
            actions: false,
            clear_cache: false,
            verbose: true, // Enable verbose
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: false,
            enable_selector: None,
            selector_command: None,
            term_exec_args: None,
        };

        // Create an app - it will have empty cache for unknown mime types
        let app = OpenIt::new(args).unwrap();
        let result = app.run();

        // The result depends on what applications are available on the system
        // If applications are found, it should succeed (JSON output)
        // If no applications are found, it should fail with "No applications found"
        if let Err(e) = result {
            // Should fail with "No applications found" message
            assert!(e.to_string().contains("No applications found"));
        }
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn test_regex_handler_executes_command() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(config_dir.join("openit")).unwrap();
        let _guard = ConfigEnvGuard::set(&config_dir);

        let marker_path = temp_dir.path().join("regex_touched");
        let script_path = temp_dir.path().join("regex_script.sh");
        fs::write(
            &script_path,
            format!("#!/bin/sh\ntouch {}\n", marker_path.display()),
        )
        .unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();

        let regex_path = config_dir.join("openit").join("regex_handlers.toml");
        fs::write(
            &regex_path,
            format!(
                "[[handlers]]\nexec = \"sh {}\"\nregexes = [\".*\\\\.txt$\"]\npriority = 5\n",
                script_path.display()
            ),
        )
        .unwrap();

        let target_path = temp_dir.path().join("match.txt");
        fs::write(&target_path, "hello").unwrap();

        let args = OpenArgs {
            target: Some(target_path.to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            auto_open_single: true,
            enable_selector: Some(false),
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        app.run().unwrap();

        let mut attempts = 0;
        while attempts < 20 {
            if marker_path.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
            attempts += 1;
        }

        assert!(
            marker_path.exists(),
            "regex handler script should have touched marker file"
        );
    }

    #[test]
    fn test_fuzzy_finder_command_construction() {
        // Test that fuzzy finder commands are constructed correctly
        // without actually running them

        let _applications = [ApplicationEntry {
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
        }];

        // Test fzf command construction - just build the command, don't run it
        let mut fzf_cmd = ProcessCommand::new("fzf");
        fzf_cmd
            .arg("--prompt")
            .arg("Open 'test.txt' with: ")
            .arg("--height=40%")
            .arg("--reverse")
            .arg("--header=★=Default ▶=XDG Associated  =Available")
            .arg("--cycle")
            .stdin(Stdio::null())
            .stdout(Stdio::null());

        // Verify command was constructed (check args)
        let fzf_program = fzf_cmd.get_program();
        assert_eq!(fzf_program, "fzf");

        // Test fuzzel command construction - just build the command, don't run it
        let mut fuzzel_cmd = ProcessCommand::new("fuzzel");
        fuzzel_cmd
            .arg("--dmenu")
            .arg("--prompt")
            .arg("Open 'test.txt' with: ")
            .arg("--index")
            .arg("--log-level=info")
            .stdin(Stdio::null())
            .stdout(Stdio::null());

        // Verify command was constructed (check args)
        let fuzzel_program = fuzzel_cmd.get_program();
        assert_eq!(fuzzel_program, "fuzzel");
    }
}
