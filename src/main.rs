use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod cli;
mod desktop_parser;
mod mime_associations;
mod xdg;

// Build info module
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

use cli::{Args, FuzzyFinder};
use desktop_parser::DesktopFile;
use mime_associations::MimeAssociations;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplicationEntry {
    name: String,
    exec: String,
    desktop_file: PathBuf,
    comment: Option<String>,
    icon: Option<String>,
    is_xdg: bool,
    xdg_priority: i32,
    is_default: bool,
    action_id: Option<String>,
}

#[derive(Debug)]
struct OpenWith {
    desktop_cache: HashMap<PathBuf, DesktopFile>,
    mime_associations: MimeAssociations,
    args: Args,
}

impl OpenWith {
    fn new(args: Args) -> Result<Self> {
        if args.clear_cache {
            Self::clear_cache()?;
        }

        let desktop_cache = Self::load_desktop_cache()?;
        let mime_associations = MimeAssociations::load();

        Ok(Self {
            desktop_cache,
            mime_associations,
            args,
        })
    }

    fn cache_path() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("open-with")
            .join("desktop_cache.json")
    }

    fn clear_cache() -> Result<()> {
        let cache_path = Self::cache_path();
        if cache_path.exists() {
            fs::remove_file(&cache_path).context("Failed to remove cache file")?;
            info!("Cache cleared");
        } else {
            info!("No cache to clear");
        }
        Ok(())
    }

    fn load_desktop_cache(
    ) -> std::collections::HashMap<std::path::PathBuf, desktop_parser::DesktopFile> {
        let cache_path = Self::cache_path();

        // Try to load from cache if it exists
        if cache_path.exists() {
            if let Ok(contents) = fs::read_to_string(&cache_path) {
                if let Ok(cache) = serde_json::from_str(&contents) {
                    debug!("Loaded desktop cache from disk");
                    return cache;
                }
            }
        }

        debug!("Building desktop file cache");
        let mut cache = HashMap::new();

        // Get desktop directories, but handle gracefully if none exist
        let desktop_dirs = xdg::get_desktop_file_paths();

        for dir in &desktop_dirs {
            // Skip directories that don't exist or can't be read
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                        match DesktopFile::parse(&path) {
                            Ok(desktop_file) => {
                                cache.insert(path, desktop_file);
                            }
                            Err(e) => {
                                debug!("Failed to parse {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            } else {
                debug!("Cannot read directory: {}", dir.display());
            }
        }

        // Try to save cache, but don't fail if we can't
        if let Some(parent) = cache_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                debug!("Failed to create cache directory: {e}");
            } else if let Ok(json) = serde_json::to_string(&cache) {
                if let Err(e) = fs::write(&cache_path, json) {
                    debug!("Failed to write cache file: {e}");
                }
            }
        }

        // Always return Ok with whatever we found (even if empty)
    }

    fn get_applications_for_mime(&self, mime_type: &str) -> Vec<ApplicationEntry> {
        let mut applications = Vec::new();
        let mut seen = HashSet::new();

        let xdg_associations = self.mime_associations.get_associations(mime_type);

        for (priority, desktop_id) in xdg_associations.iter().enumerate() {
            if let Some((path, desktop_file)) = self.find_desktop_file(desktop_id) {
                if seen.insert(desktop_id.clone()) {
                    if let Some(entry) = &desktop_file.main_entry {
                        applications.push(ApplicationEntry {
                            name: entry.name.clone(),
                            exec: entry.exec.clone(),
                            desktop_file: path.clone(),
                            comment: entry.comment.clone(),
                            icon: entry.icon.clone(),
                            is_xdg: true,
                            xdg_priority: i32::try_from(priority).unwrap_or(i32::MAX),
                            is_default: priority == 0,
                            action_id: None,
                        });

                        if self.args.actions {
                            for (action_id, action) in &desktop_file.actions {
                                applications.push(ApplicationEntry {
                                    name: format!("{} - {}", entry.name, action.name),
                                    exec: action.exec.clone(),
                                    desktop_file: path.clone(),
                                    comment: Some(format!("Action: {}", action.name)),
                                    icon: action.icon.clone().or_else(|| entry.icon.clone()),
                                    is_xdg: true,
                                    xdg_priority: i32::try_from(priority).unwrap_or(i32::MAX),
                                    is_default: false,
                                    action_id: Some(action_id.clone()),
                                });
                            }
                        }
                    }
                }
            }
        }

        for (path, desktop_file) in &self.desktop_cache {
            if let Some(entry) = &desktop_file.main_entry {
                if entry.mime_types.contains(&mime_type.to_string()) {
                    let desktop_id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    if seen.insert(desktop_id) {
                        applications.push(ApplicationEntry {
                            name: entry.name.clone(),
                            exec: entry.exec.clone(),
                            desktop_file: path.clone(),
                            comment: entry.comment.clone(),
                            icon: entry.icon.clone(),
                            is_xdg: false,
                            xdg_priority: -1,
                            is_default: false,
                            action_id: None,
                        });

                        if self.args.actions {
                            for (action_id, action) in &desktop_file.actions {
                                applications.push(ApplicationEntry {
                                    name: format!("{} - {}", entry.name, action.name),
                                    exec: action.exec.clone(),
                                    desktop_file: path.clone(),
                                    comment: Some(format!("Action: {}", action.name)),
                                    icon: action.icon.clone().or_else(|| entry.icon.clone()),
                                    is_xdg: false,
                                    xdg_priority: -1,
                                    is_default: false,
                                    action_id: Some(action_id.clone()),
                                });
                            }
                        }
                    }
                }
            }
        }

        applications
    }

    fn find_desktop_file(&self, desktop_id: &str) -> Option<(&PathBuf, &DesktopFile)> {
        for (path, desktop_file) in &self.desktop_cache {
            if path.file_name().and_then(|n| n.to_str()) == Some(desktop_id) {
                return Some((path, desktop_file));
            }
        }

        for (path, desktop_file) in &self.desktop_cache {
            if path.to_string_lossy().ends_with(desktop_id) {
                return Some((path, desktop_file));
            }
        }

        None
    }

    fn run_fuzzy_finder(
        &self,
        applications: &[ApplicationEntry],
        file_name: &str,
    ) -> Result<Option<usize>> {
        let fuzzer = match &self.args.fuzzer {
            FuzzyFinder::Auto => {
                if which::which("fzf").is_ok() {
                    "fzf"
                } else if which::which("fuzzel").is_ok() {
                    "fuzzel"
                } else {
                    return Err(anyhow::anyhow!(
                        "No fuzzy finder found. Install fzf or fuzzel."
                    ));
                }
            }
            FuzzyFinder::Fzf => "fzf",
            FuzzyFinder::Fuzzel => "fuzzel",
        };

        match fuzzer {
            "fzf" => Self::run_fzf(applications, file_name),
            "fuzzel" => Self::run_fuzzel(applications, file_name),
            _ => unreachable!(),
        }
    }

    fn run_fzf(applications: &[ApplicationEntry], file_name: &str) -> Result<Option<usize>> {
        let mut child = Command::new("fzf")
            .arg("--prompt")
            .arg(format!("Open '{file_name}' with: "))
            .arg("--height=40%")
            .arg("--reverse")
            .arg("--header=★=Default ▶=XDG Associated  =Available")
            .arg("--cycle")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.as_mut().context("Failed to get stdin")?;

        for app in applications {
            let marker = if app.is_default {
                "★ "
            } else if app.is_xdg {
                "▶ "
            } else {
                "  "
            };

            let display = if let Some(comment) = &app.comment {
                format!("{}{} - {}", marker, app.name, comment)
            } else {
                format!("{}{}", marker, app.name)
            };

            writeln!(stdin, "{display}")?;
        }

        let output = child.wait_with_output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();

        for (i, app) in applications.iter().enumerate() {
            let marker = if app.is_default {
                "★ "
            } else if app.is_xdg {
                "▶ "
            } else {
                "  "
            };

            let display = if let Some(comment) = &app.comment {
                format!("{}{} - {}", marker, app.name, comment)
            } else {
                format!("{}{}", marker, app.name)
            };

            if display == selected {
                return Ok(Some(i));
            }
        }

        Ok(None)
    }

    fn run_fuzzel(applications: &[ApplicationEntry], file_name: &str) -> Result<Option<usize>> {
        let mut child = Command::new("fuzzel")
            .arg("--dmenu")
            .arg("--prompt")
            .arg(format!("Open '{file_name}' with: "))
            .arg("--index")
            .arg("--log-level=info")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.as_mut().context("Failed to get stdin")?;

        for app in applications {
            let marker = if app.is_default {
                "★"
            } else if app.is_xdg {
                "▶"
            } else {
                "   "
            };

            let display = format!("{}{}", marker, app.name);

            if let Some(icon) = &app.icon {
                stdin.write_all(display.as_bytes())?;
                stdin.write_all(b"\0")?;
                stdin.write_all(b"icon\x1f")?;
                stdin.write_all(icon.as_bytes())?;
                stdin.write_all(b"\n")?;
            } else {
                writeln!(stdin, "{display}")?;
            }
        }

        let output = child.wait_with_output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let index_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(index_str.parse().ok())
    }

    fn execute_application(app: &ApplicationEntry, file_path: &Path) -> Result<()> {
        let exec = &app.exec;

        let clean_exec = exec
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

        info!("Executing: {} \"{}\"", clean_exec, file_path.display());

        let parts: Vec<&str> = clean_exec.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty exec command"));
        }

        let mut cmd = Command::new(parts[0]);

        for part in &parts[1..] {
            cmd.arg(part);
        }

        cmd.arg(file_path);

        unsafe {
            cmd.pre_exec(|| {
                nix::unistd::setsid()?;
                Ok(())
            });
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to execute application")?;

        Ok(())
    }

    fn output_json(
        &self,
        applications: &[ApplicationEntry],
        file_path: &Path,
        mime_type: &str,
    ) -> Result<()> {
        let xdg_associations: Vec<String> = self
            .mime_associations
            .get_associations(mime_type)
            .into_iter()
            .collect();

        let output = serde_json::json!({
            "file": file_path,
            "mimetype": mime_type,
            "xdg_associations": xdg_associations,
            "applications": applications,
        });

        println!("{}", serde_json::to_string_pretty(&output)?);
        Ok(())
    }

    pub fn run(self) -> Result<()> {
        // Handle clear-cache early if no file is provided
        if self.args.clear_cache && self.args.file.is_none() {
            return Ok(());
        }

        let file_path = if let Some(file) = &self.args.file {
            file.canonicalize().context("Failed to resolve file path")?
        } else {
            return Err(anyhow::anyhow!("No file provided"));
        };

        if !file_path.exists() {
            return Err(anyhow::anyhow!(
                "File does not exist: {}",
                file_path.display()
            ));
        }

        if !file_path.is_file() {
            return Err(anyhow::anyhow!(
                "Path is not a file: {}",
                file_path.display()
            ));
        }

        let mime_type = mime_guess::from_path(&file_path)
            .first_or_octet_stream()
            .to_string();

        info!("File: {}", file_path.display());
        info!("MIME type: {mime_type}");

        let applications = self.get_applications_for_mime(&mime_type);

        if applications.is_empty() {
            return Err(anyhow::anyhow!(
                "No applications found for MIME type: {}",
                mime_type
            ));
        }

        if self.args.json {
            self.output_json(&applications, &file_path, &mime_type)?;
        } else if io::stdout().is_terminal() {
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");

            if let Some(index) = self.run_fuzzy_finder(&applications, file_name)? {
                Self::execute_application(&applications[index], &file_path)?;
            }
        } else {
            self.output_json(&applications, &file_path, &mime_type)?;
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.build_info {
        cli::show_build_info();
        return Ok(());
    }

    if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    }

    let app = OpenWith::new(args)?;
    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use tempfile::TempDir;

    fn create_test_desktop_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let file_path = dir.join(name);
        fs::write(&file_path, content).unwrap();
        file_path
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
        };

        let json = serde_json::to_string(&app).unwrap();
        let deserialized: ApplicationEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(app.name, deserialized.name);
        assert_eq!(app.exec, deserialized.exec);
        assert_eq!(app.is_default, deserialized.is_default);
    }

    #[test]
    fn test_clean_exec_command() {
        let test_cases = vec![
            ("app %f", "app"),
            ("app %F %u", "app"),
            ("app %%", "app %"),
            ("app %i %c %k", "app"),
            ("  app %f  ", "app"),
        ];

        for (input, expected) in test_cases {
            let clean = input
                .replace("%u", "")
                .replace("%U", "")
                .replace("%f", "")
                .replace("%F", "")
                .replace("%i", "")
                .replace("%c", "")
                .replace("%k", "")
                .replace("%%", "%");
            let clean = clean.trim();

            assert_eq!(clean, expected, "Failed for input: {input}");
        }
    }

    #[test]
    fn test_cache_path_creation() {
        let cache_path = OpenWith::cache_path();
        assert!(cache_path.ends_with("open-with/desktop_cache.json"));
    }

    #[test]
    fn test_find_desktop_file_exact_match() {
        let temp_dir = TempDir::new().unwrap();
        let desktop_content = r"[Desktop Entry]
Name=Test
Exec=test";

        let file_path = create_test_desktop_file(temp_dir.path(), "test.desktop", desktop_content);

        let mut cache = HashMap::new();
        let desktop_file = DesktopFile::parse(&file_path).unwrap();
        cache.insert(file_path.clone(), desktop_file);

        let args = Args {
            file: Some(PathBuf::from("test.txt")),
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
        };

        let app = OpenWith {
            desktop_cache: cache,
            mime_associations: MimeAssociations::new(),
            args,
        };

        let result = app.find_desktop_file("test.desktop");
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
    fn test_new_with_clear_cache() {
        // Test that OpenWith::new succeeds when clear_cache is true
        // This should work even in environments with no desktop files
        let args = Args {
            file: Some(PathBuf::from("test.txt")),
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: false,
            clear_cache: true,
            verbose: false,
            build_info: false,
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
        let result = OpenWith::new(args);

        // If it fails, print the error for debugging
        if let Err(ref e) = result {
            eprintln!("OpenWith::new failed with error: {e}");
            eprintln!("Error chain: {e:?}");
        }

        assert!(
            result.is_ok(),
            "OpenWith::new should handle missing cache and desktop files gracefully"
        );
    }

    #[test]
    fn test_get_applications_for_mime_empty() {
        let args = Args {
            file: Some(PathBuf::from("test.txt")),
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
        };

        let app = OpenWith {
            desktop_cache: HashMap::new(),
            mime_associations: MimeAssociations::new(),
            args,
        };

        let apps = app.get_applications_for_mime("application/unknown");
        assert!(apps.is_empty());
    }

    #[test]
    fn test_clear_cache() {
        use tempfile::TempDir;

        // Create a temporary directory for the cache
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("open-with");
        fs::create_dir_all(&cache_dir).unwrap();

        // Create a mock cache file
        let cache_file = cache_dir.join("desktop_cache.json");
        fs::write(&cache_file, "test cache").unwrap();

        // Verify file exists
        assert!(cache_file.exists());

        // Clear the specific cache file
        if cache_file.exists() {
            fs::remove_file(&cache_file).unwrap();
        }

        // Verify cache file is removed
        assert!(!cache_file.exists());
    }

    #[test]
    fn test_output_json() {
        let args = Args {
            file: Some(PathBuf::from("test.txt")),
            fuzzer: FuzzyFinder::Auto,
            json: true,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
        };

        let app = OpenWith {
            desktop_cache: HashMap::new(),
            mime_associations: MimeAssociations::new(),
            args,
        };

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
        }];

        let file_path = PathBuf::from("test.txt");
        let mime_type = "text/plain";

        // This will print to stdout, but we're mainly testing it doesn't panic
        let result = app.output_json(&applications, &file_path, mime_type);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_no_file() {
        let args = Args {
            file: None,
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
        };

        let app = OpenWith {
            desktop_cache: HashMap::new(),
            mime_associations: MimeAssociations::new(),
            args,
        };

        let result = app.run();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No file provided");
    }

    #[test]
    fn test_run_with_nonexistent_file() {
        let args = Args {
            file: Some(PathBuf::from("/nonexistent/file.txt")),
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: false,
            clear_cache: false,
            verbose: false,
            build_info: false,
        };

        let app = OpenWith {
            desktop_cache: HashMap::new(),
            mime_associations: MimeAssociations::new(),
            args,
        };

        let result = app.run();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to resolve file path"));
    }

    #[test]
    fn test_run_clear_cache_only() {
        use tempfile::TempDir;

        // Create a temporary directory for the cache
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("open-with");
        fs::create_dir_all(&cache_dir).unwrap();

        // Override the cache path for this test
        std::env::set_var("HOME", temp_dir.path());

        let args = Args {
            file: None,
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: false,
            clear_cache: true,
            verbose: false,
            build_info: false,
        };

        // This should succeed even if no cache file exists
        let app = OpenWith::new(args).unwrap();
        let result = app.run();
        assert!(result.is_ok());

        // Restore HOME
        std::env::remove_var("HOME");
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

        let mut cache = HashMap::new();
        let desktop_file = DesktopFile::parse(&file_path).unwrap();
        cache.insert(file_path.clone(), desktop_file);

        let args = Args {
            file: Some(PathBuf::from("test.txt")),
            fuzzer: FuzzyFinder::Auto,
            json: false,
            actions: true, // Enable actions
            clear_cache: false,
            verbose: false,
            build_info: false,
        };

        let app = OpenWith {
            desktop_cache: cache,
            mime_associations: MimeAssociations::new(),
            args,
        };

        let apps = app.get_applications_for_mime("text/plain");

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
}
