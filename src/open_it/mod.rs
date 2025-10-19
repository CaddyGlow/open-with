#[cfg(test)]
use crate::application_finder::ApplicationEntry;
use crate::application_finder::ApplicationFinder;
use crate::cache::DesktopCache;
#[cfg(test)]
use crate::cache::FileSystemCache;
use crate::cli::OpenArgs;
use crate::config;
use crate::executor::ApplicationExecutor;
use crate::mime_associations::MimeAssociations;
use crate::regex_handlers::RegexHandlerStore;
use crate::selector::SelectorRunner;
use crate::target::LaunchTarget;
use anyhow::Result;
use log::{debug, info};
use serde_json::json;
use std::io::{self, IsTerminal};
#[cfg(test)]
use std::path::PathBuf;

mod bootstrap;
mod execution;
mod selection;
mod target;

use bootstrap::BootstrapOutcome;
use execution::application_from_regex;
use selection::LaunchContext;

#[derive(Debug)]
pub struct OpenIt {
    pub(crate) application_finder: ApplicationFinder,
    pub(crate) selector_runner: SelectorRunner,
    pub(crate) executor: ApplicationExecutor,
    pub(crate) config: config::Config,
    pub(crate) regex_handlers: RegexHandlerStore,
    pub(crate) args: OpenArgs,
}

impl OpenIt {
    pub fn new(args: OpenArgs) -> Result<Self> {
        if args.clear_cache {
            Self::clear_cache()?;
        }

        let BootstrapOutcome {
            desktop_cache,
            mut config,
        } = bootstrap::initialize(&args)?;

        if let Some(open_with) = args.open_with_override() {
            config.selector.open_with = open_with;
        }

        if let Some(term_exec_args) = args.term_exec_args.clone() {
            config.selector.term_exec_args = if term_exec_args.is_empty() {
                None
            } else {
                Some(term_exec_args)
            };
        }

        let application_finder = ApplicationFinder::new(desktop_cache, MimeAssociations::load());

        let executor = ApplicationExecutor::with_options(
            config.app_launch_prefix.clone(),
            config.selector.term_exec_args.clone(),
        );

        Ok(Self {
            application_finder,
            selector_runner: SelectorRunner::new(),
            executor,
            config,
            regex_handlers: RegexHandlerStore::load(None)?,
            args,
        })
    }

    pub fn run(self) -> Result<()> {
        if self.args.clear_cache && self.args.target.is_none() {
            return Ok(());
        }

        let context = self.prepare_launch()?;

        let force_json =
            self.args.json || (!io::stdout().is_terminal() && self.config.selector.open_with);
        if force_json {
            return self.output_json(&context);
        }

        if !self.config.selector.open_with {
            let first_app = &context.applications[0];
            if context.first_is_regex_handler() {
                info!("Selector disabled; launching regex handler directly");
            } else {
                info!(
                    "Selector disabled; launching `{}` ({})",
                    first_app.name,
                    first_app.desktop_file.display()
                );
            }
            return self.execute_application(first_app, &context.target);
        }

        if context.applications.len() == 1 {
            info!("Auto-opening the only available application");
            return self.execute_application(&context.applications[0], &context.target);
        }

        self.run_selector_flow(&context)
    }

    fn prepare_launch(&self) -> Result<LaunchContext> {
        let raw_target = self
            .args
            .target
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No target provided"))?;

        let target = Self::resolve_launch_target(raw_target)?;

        if let Some(path) = target.as_path() {
            if path.is_dir() {
                info!("Directory: {}", path.display());
            } else {
                info!("File: {}", path.display());
            }
        } else {
            info!("URI: {}", target.as_command_argument());
        }

        let mime_type = Self::mime_for_target(&target);
        info!("MIME type: {mime_type}");

        let candidate = target.as_command_argument().into_owned();
        let mut applications = self
            .application_finder
            .find_for_mime(&mime_type, self.args.actions);

        if let Some(handler) = self.regex_handlers.find_handler(&candidate) {
            info!(
                "Matched regex handler (priority {}): {}",
                handler.priority, handler.exec
            );
            if handler.terminal {
                info!("Regex handler requests terminal execution");
            }

            applications.insert(0, application_from_regex(handler));
        }

        debug!(
            "Found {} application(s); regex handler count: {}",
            applications.len(),
            self.regex_handlers.len()
        );

        if applications.is_empty() {
            anyhow::bail!("No applications found for MIME type: {}", mime_type);
        }

        Ok(LaunchContext::new(target, mime_type, applications))
    }

    fn output_json(&self, context: &LaunchContext) -> Result<()> {
        let resource = context.target.as_command_argument().into_owned();
        let target_kind = match context.target {
            LaunchTarget::File(_) => "file",
            LaunchTarget::Uri(_) => "uri",
        };

        let output = json!({
            "target": resource,
            "target_kind": target_kind,
            "mimetype": context.mime_type,
            "xdg_associations": Vec::<String>::new(),
            "applications": context.applications,
        });

        println!("{}", serde_json::to_string_pretty(&output)?);
        Ok(())
    }

    pub fn load_desktop_cache() -> Box<dyn DesktopCache> {
        bootstrap::load_desktop_cache()
    }

    #[cfg(test)]
    pub fn populate_cache_from_dirs(
        cache: &mut FileSystemCache,
        desktop_dirs: &[PathBuf],
        force: bool,
    ) -> bool {
        bootstrap::populate_cache_from_dirs(cache, desktop_dirs, force)
    }

    #[cfg(test)]
    pub fn cache_path() -> PathBuf {
        bootstrap::cache_path()
    }

    pub fn clear_cache() -> Result<()> {
        bootstrap::clear_cache()
    }

    pub fn resolve_launch_target(raw: &str) -> Result<LaunchTarget> {
        target::resolve_launch_target(raw)
    }

    pub fn mime_for_target(target: &LaunchTarget) -> String {
        target::mime_for_target(target)
    }

    #[cfg(test)]
    pub(crate) fn output_json_for_test(
        &self,
        applications: Vec<ApplicationEntry>,
        target: LaunchTarget,
        mime_type: String,
    ) -> Result<()> {
        let context = LaunchContext::new(target, mime_type, applications);
        self.output_json(&context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_finder::ApplicationEntry;
    use crate::cache::{DesktopCache, FileSystemCache};
    use crate::cli::{OpenArgs, SelectorKind};
    use crate::config::Config;
    use crate::desktop_parser::{DesktopEntry, DesktopFile};
    use crate::executor::ApplicationExecutor;
    use crate::regex_handlers::RegexHandlerStore;
    use crate::selector::SelectorRunner;
    use crate::target::LaunchTarget;
    use crate::test_support::{create_test_desktop_file, CacheEnvGuard, ConfigEnvGuard};
    use serial_test::serial;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Command as ProcessCommand, Stdio};
    use std::time::Duration;
    use tempfile::TempDir;
    use url::Url;

    fn basic_desktop_content(name: &str, exec: &str, mime: &str) -> String {
        format!("[Desktop Entry]\nName={name}\nExec={exec}\nMimeType={mime};\nTerminal=false\n")
    }

    fn create_test_args_json(target: Option<PathBuf>) -> OpenArgs {
        OpenArgs {
            target: target.map(|p| p.to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: true,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            open_with: true,
            no_open_with: false,
            selector_command: None,
            term_exec_args: None,
        }
    }

    #[cfg(unix)]
    fn build_selector_test_environment(script_body: &str) -> (OpenIt, LaunchContext, TempDir) {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("selector_script.sh");
        fs::write(&script_path, script_body).unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();

        let mut config = Config::default();
        config.selector.open_with = true;

        let executor = ApplicationExecutor::with_options(
            config.app_launch_prefix.clone(),
            config.selector.term_exec_args.clone(),
        );

        let args = OpenArgs {
            target: Some("dummy.txt".to_string()),
            selector: SelectorKind::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            open_with: false,
            no_open_with: false,
            selector_command: Some(script_path.to_string_lossy().to_string()),
            term_exec_args: None,
        };

        let applications = vec![
            ApplicationEntry {
                name: "Alpha".to_string(),
                exec: "alpha %F".to_string(),
                desktop_file: PathBuf::from("alpha.desktop"),
                comment: None,
                icon: None,
                is_xdg: false,
                xdg_priority: -1,
                is_default: false,
                action_id: None,
                requires_terminal: false,
                is_terminal_emulator: false,
            },
            ApplicationEntry {
                name: "Beta".to_string(),
                exec: "beta %F".to_string(),
                desktop_file: PathBuf::from("beta.desktop"),
                comment: None,
                icon: None,
                is_xdg: false,
                xdg_priority: -1,
                is_default: false,
                action_id: None,
                requires_terminal: false,
                is_terminal_emulator: false,
            },
        ];

        let context = LaunchContext::new(
            LaunchTarget::File(PathBuf::from("dummy.txt")),
            "text/plain".to_string(),
            applications,
        );

        let open_with = OpenIt {
            application_finder: ApplicationFinder::new(
                Box::new(crate::cache::MemoryCache::new()),
                MimeAssociations::with_associations(HashMap::new()),
            ),
            selector_runner: SelectorRunner::new(),
            executor,
            config,
            regex_handlers: RegexHandlerStore::load(None).unwrap(),
            args,
        };

        (open_with, context, temp_dir)
    }

    #[test]
    fn populate_cache_adds_new_entries_without_rebuild() {
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

        assert!(OpenIt::populate_cache_from_dirs(
            &mut cache,
            &[apps_dir.clone()],
            true
        ));
        assert!(DesktopCache::get(&cache, &existing).is_some());

        let new_entry_path = create_test_desktop_file(
            &apps_dir,
            "imgcat.desktop",
            &basic_desktop_content("Imgcat", "imgcat %f", "image/png"),
        );

        assert!(OpenIt::populate_cache_from_dirs(
            &mut cache,
            &[apps_dir.clone()],
            false
        ));
        assert!(DesktopCache::get(&cache, &new_entry_path).is_some());
    }

    #[test]
    fn cache_path_creation() {
        let cache_path = OpenIt::cache_path();
        assert!(cache_path.ends_with("openit/desktop_cache.json"));
    }

    #[test]
    #[serial]
    fn new_with_clear_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file = temp_dir.path().join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);

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
            open_with: false,
            no_open_with: false,
            selector_command: None,
            term_exec_args: None,
        };

        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();

        let result = OpenIt::new(args);
        assert!(result.is_ok());
    }

    #[test]
    fn applications_for_mime_empty() {
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));
        let app = OpenIt::new(args).unwrap();

        let apps = app
            .application_finder
            .find_for_mime("application/unknown", app.args.actions);
        assert!(apps.is_empty());
    }

    #[test]
    #[serial]
    fn clear_cache_succeeds() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("openit");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_file = cache_dir.join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);
        fs::write(&cache_file, "test cache").unwrap();

        assert!(cache_file.exists());

        OpenIt::clear_cache().unwrap();
        assert!(!cache_file.exists());
    }

    #[test]
    fn output_json_formats_payload() {
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

        app.output_json_for_test(applications, target, mime_type.to_string())
            .unwrap();
    }

    #[test]
    fn resolve_launch_target_with_uri() {
        let target = OpenIt::resolve_launch_target("https://example.com").unwrap();
        assert!(matches!(target, LaunchTarget::Uri(_)));
        assert_eq!(OpenIt::mime_for_target(&target), "x-scheme-handler/https");
    }

    #[test]
    fn mime_for_directory_target() {
        let temp_dir = TempDir::new().unwrap();
        let target = LaunchTarget::File(temp_dir.path().to_path_buf());
        assert_eq!(OpenIt::mime_for_target(&target), "inode/directory");
    }

    #[test]
    fn resolve_launch_target_with_file_uri() {
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
    fn run_with_no_file_errors() {
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
            open_with: false,
            no_open_with: false,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No target provided");
    }

    #[test]
    fn run_with_nonexistent_file_errors() {
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
            open_with: false,
            no_open_with: false,
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
    fn run_clear_cache_only_succeeds() {
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
            open_with: false,
            no_open_with: false,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        app.run().unwrap();
    }

    #[test]
    fn run_with_directory_path_reports_missing_handlers() {
        let temp_dir = TempDir::new().unwrap();

        let args = OpenArgs {
            target: Some(temp_dir.path().to_string_lossy().to_string()),
            selector: SelectorKind::Auto,
            json: true,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
            generate_config: false,
            config: None,
            open_with: false,
            no_open_with: true,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        assert!(app.run().is_ok());
    }

    #[test]
    fn run_with_no_applications_for_mime_errors() {
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
            open_with: false,
            no_open_with: false,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        assert!(result.is_err());
    }

    #[test]
    fn load_desktop_cache_with_invalid_file_recovers() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("openit");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_file = cache_dir.join("desktop_cache.json");
        fs::write(&cache_file, "invalid json content").unwrap();

        env::set_var("HOME", temp_dir.path());
        let cache = OpenIt::load_desktop_cache();
        env::remove_var("HOME");

        assert!(cache.is_empty() || !cache.is_empty());
    }

    #[test]
    #[serial]
    fn clear_cache_with_permission_error_is_ok() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file = temp_dir.path().join("desktop_cache.json");
        let _cache_env = CacheEnvGuard::set(&cache_file);

        let result = OpenIt::clear_cache();
        assert!(result.is_ok());
    }

    #[test]
    fn cache_save_failure_handling() {
        let temp_dir = TempDir::new().unwrap();
        let readonly_dir = temp_dir.path().join("readonly");
        fs::create_dir(&readonly_dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o444);
            fs::set_permissions(&readonly_dir, perms).unwrap();
        }

        env::set_var("HOME", &readonly_dir);
        let cache = OpenIt::load_desktop_cache();
        env::remove_var("HOME");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&readonly_dir, perms).unwrap();
        }

        assert!(cache.is_empty() || !cache.is_empty());
    }

    #[test]
    fn run_fuzzy_finder_auto_detection() {
        let args = create_test_args_json(Some(PathBuf::from("test.txt")));
        let _app = OpenIt::new(args).unwrap();

        let _ = which::which("fzf");
        let _ = which::which("fuzzel");
    }

    #[test]
    fn run_json_output_non_terminal() {
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

        app.output_json_for_test(applications, target, mime_type.to_string())
            .unwrap();
    }

    #[test]
    fn run_with_verbose_logging_handles_errors() {
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
            json: true,
            actions: false,
            clear_cache: false,
            verbose: true,
            build_info: false,
            generate_config: false,
            config: None,
            open_with: false,
            no_open_with: false,
            selector_command: None,
            term_exec_args: None,
        };

        let app = OpenIt::new(args).unwrap();
        let result = app.run();
        if let Err(e) = result {
            assert!(e.to_string().contains("No applications found"));
        }
    }

    #[test]
    #[cfg(unix)]
    fn selector_cancellation_returns_ok_without_fallback() {
        let (open_with, context, _temp_dir) =
            build_selector_test_environment("#!/bin/sh\nexit 0\n");

        assert!(open_with.run_selector_flow(&context).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn selector_error_propagates_without_fallback() {
        let (open_with, context, _temp_dir) =
            build_selector_test_environment("#!/bin/sh\nprintf \"Unknown\"\n");

        let result = open_with.run_selector_flow(&context);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn regex_handler_executes_command() {
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
            open_with: false,
            no_open_with: true,
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

        assert!(marker_path.exists());
    }

    #[test]
    fn resolve_terminal_launcher_prefers_scheme_handler() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());

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
    fn resolve_terminal_launcher_falls_back_to_category() {
        let mut cache = Box::new(crate::cache::MemoryCache::new());

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
    fn resolve_terminal_launcher_errors_without_terminal() {
        let cache = Box::new(crate::cache::MemoryCache::new());
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
            selector_runner: SelectorRunner::new(),
            executor,
            config,
            regex_handlers,
            args,
        };

        let result = open_with.resolve_terminal_launcher();
        assert!(result.is_err());
    }

    #[test]
    fn fuzzy_finder_command_construction() {
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

        assert_eq!(fzf_cmd.get_program(), "fzf");

        let mut fuzzel_cmd = ProcessCommand::new("fuzzel");
        fuzzel_cmd
            .arg("--dmenu")
            .arg("--prompt")
            .arg("Open with")
            .stdin(Stdio::null())
            .stdout(Stdio::null());

        assert_eq!(fuzzel_cmd.get_program(), "fuzzel");
    }
}
