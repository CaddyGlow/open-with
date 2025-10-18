use crate::application_finder::ApplicationEntry;
use crate::config::Config;
use crate::template::TemplateEngine;
use anyhow::{Context, Result};
use log::info;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Default)]
pub struct FuzzyFinderRunner;

impl FuzzyFinderRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run(
        &self,
        config: &Config,
        applications: &[ApplicationEntry],
        file_name: &str,
        fuzzer_name: &str,
    ) -> Result<Option<usize>> {
        let profile = config.get_selector_profile(fuzzer_name).ok_or_else(|| {
            anyhow::anyhow!("No configuration found for fuzzy finder: {}", fuzzer_name)
        })?;

        // Create template engine for substitutions
        let mut template_engine = TemplateEngine::new();
        template_engine.set("file", file_name);

        let prompt = template_engine.render(config.get_prompt_template(profile));
        let header = config.get_header_template(profile);

        template_engine.set("prompt", &prompt).set("header", header);

        let mut cmd = Command::new(&profile.command);

        // Apply template substitutions to args using template engine
        let substituted_args = template_engine.render_args(&profile.args);
        let mut logged_args = Vec::new();
        for arg in substituted_args {
            logged_args.push(arg.clone());
            cmd.arg(arg);
        }

        // Set any environment variables
        for (key, value) in &profile.env {
            cmd.env(key, value);
        }

        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        let log_command = if logged_args.is_empty() {
            profile.command.clone()
        } else {
            format!("{} {}", profile.command, logged_args.join(" "))
        };
        info!("Launching fuzzy finder: {}", log_command);

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.as_mut().context("Failed to get stdin")?;

        // Write entries using configurable templates
        for app in applications {
            let marker = if app.is_default {
                config.get_marker(profile, "default")
            } else if app.is_xdg {
                config.get_marker(profile, "xdg")
            } else {
                config.get_marker(profile, "available")
            };

            let comment = app
                .comment
                .as_ref()
                .map_or(String::new(), |c| format!(" - {c}"));

            // Use template engine for entry rendering
            let mut entry_template_engine = TemplateEngine::new();
            entry_template_engine
                .set("marker", marker)
                .set("name", &app.name)
                .set("comment", &comment);

            let display = entry_template_engine.render(&profile.entry_template);
            writeln!(stdin, "{display}")?;
        }

        let output = child.wait_with_output()?;

        if !output.status.success() {
            info!(
                "Fuzzy finder `{}` exited with status {:?}",
                profile.command,
                output.status.code()
            );
            return Ok(None);
        }

        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Handle fuzzel's index output
        if profile.command == "fuzzel" && profile.args.contains(&"--index".to_string()) {
            let parsed: Option<usize> = selected.parse().ok();
            if let Some(idx) = parsed {
                if let Some(app) = applications.get(idx) {
                    info!(
                        "Fuzzy finder `{}` selected `{}` ({})",
                        profile.command,
                        app.name,
                        app.desktop_file.display()
                    );
                } else {
                    info!(
                        "Fuzzy finder `{}` reported selection index {}",
                        profile.command, idx
                    );
                }
            }
            return Ok(parsed);
        }

        // Generic matching for other fuzzy finders
        for (i, app) in applications.iter().enumerate() {
            let marker = if app.is_default {
                config.get_marker(profile, "default")
            } else if app.is_xdg {
                config.get_marker(profile, "xdg")
            } else {
                config.get_marker(profile, "available")
            };

            let comment = app
                .comment
                .as_ref()
                .map_or(String::new(), |c| format!(" - {c}"));

            // Use template engine for entry matching
            let mut entry_template_engine = TemplateEngine::new();
            entry_template_engine
                .set("marker", marker)
                .set("name", &app.name)
                .set("comment", &comment);

            let display = entry_template_engine.render(&profile.entry_template);

            if display == selected {
                info!(
                    "Fuzzy finder `{}` selected `{}` ({})",
                    profile.command,
                    app.name,
                    app.desktop_file.display()
                );
                return Ok(Some(i));
            }
        }

        Ok(None)
    }

    pub fn detect_available(&self, config: &Config) -> Result<String> {
        if which::which("fzf").is_ok() && config.get_selector_profile("fzf").is_some() {
            Ok("fzf".to_string())
        } else if which::which("fuzzel").is_ok() && config.get_selector_profile("fuzzel").is_some()
        {
            Ok("fuzzel".to_string())
        } else {
            Err(anyhow::anyhow!(
                "No fuzzy finder found. Install fzf or fuzzel."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_finder::ApplicationEntry;
    use std::path::PathBuf;

    fn create_test_application() -> ApplicationEntry {
        ApplicationEntry {
            name: "Test App".to_string(),
            exec: "testapp %F".to_string(),
            desktop_file: PathBuf::from("/usr/share/applications/testapp.desktop"),
            comment: Some("Test application".to_string()),
            icon: Some("testapp-icon".to_string()),
            is_xdg: true,
            xdg_priority: 0,
            is_default: true,
            action_id: None,
        }
    }

    #[test]
    fn test_new_fuzzy_finder_runner() {
        let config = Config::default();
        let _runner = FuzzyFinderRunner::new();

        // Should have default fuzzy finder configs
        assert!(config.get_selector_profile("fzf").is_some());
        assert!(config.get_selector_profile("fuzzel").is_some());
    }

    #[test]
    fn test_detect_available_with_fzf() {
        let config = Config::default();
        let runner = FuzzyFinderRunner::new();

        // This test depends on system state, so we'll test the logic
        let result = runner.detect_available(&config);

        // Should either succeed with a fuzzy finder name or fail with appropriate error
        match result {
            Ok(name) => {
                assert!(name == "fzf" || name == "fuzzel");
            }
            Err(e) => {
                assert!(e.to_string().contains("No fuzzy finder found"));
            }
        }
    }

    #[test]
    fn test_run_with_invalid_fuzzer() {
        let config = Config::default();
        let runner = FuzzyFinderRunner::new();
        let _applications = vec![create_test_application()];

        let result = runner.run(&config, &_applications, "test.txt", "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No configuration found for fuzzy finder"));
    }

    #[test]
    fn test_run_command_construction() {
        // Test that we can construct the command without actually running it
        let config = Config::default();
        let runner = FuzzyFinderRunner::new();
        let _applications = vec![create_test_application()];

        // We can't easily test the actual execution without mocking,
        // but we can test that the function handles the setup correctly
        let profile = config.get_selector_profile("fzf").unwrap();

        // Test template engine setup
        let mut template_engine = TemplateEngine::new();
        template_engine.set("file", "test.txt");

        let prompt = template_engine.render(config.get_prompt_template(profile));
        assert!(prompt.contains("test.txt"));

        let header = config.get_header_template(profile);
        assert!(!header.is_empty());

        template_engine.set("prompt", &prompt).set("header", header);

        let substituted_args = template_engine.render_args(&profile.args);
        assert!(!substituted_args.is_empty());

        // Verify template substitution worked
        let prompt_arg_found = substituted_args.iter().any(|arg| arg.contains("test.txt"));
        assert!(prompt_arg_found);
    }

    #[test]
    fn test_entry_display_generation() {
        let config = Config::default();
        let _runner = FuzzyFinderRunner::new();
        let app = create_test_application();
        let profile = config.get_selector_profile("fzf").unwrap();

        let marker = if app.is_default {
            config.get_marker(profile, "default")
        } else if app.is_xdg {
            config.get_marker(profile, "xdg")
        } else {
            config.get_marker(profile, "available")
        };

        let comment = app
            .comment
            .as_ref()
            .map_or(String::new(), |c| format!(" - {c}"));

        let mut entry_template_engine = TemplateEngine::new();
        entry_template_engine
            .set("marker", marker)
            .set("name", &app.name)
            .set("comment", &comment);

        let display = entry_template_engine.render(&profile.entry_template);

        assert!(display.contains("Test App"));
        assert!(display.contains("Test application"));
        assert!(!display.is_empty());
    }

    #[test]
    fn test_fuzzel_index_parsing() {
        // Test the fuzzel index parsing logic
        let selected = "2";
        let parsed: Option<usize> = selected.parse().ok();
        assert_eq!(parsed, Some(2));

        let invalid = "not_a_number";
        let parsed: Option<usize> = invalid.parse().ok();
        assert_eq!(parsed, None);
    }

    #[test]
    fn test_entry_matching_logic() {
        let config = Config::default();
        let _runner = FuzzyFinderRunner::new();
        let applications = vec![
            create_test_application(),
            ApplicationEntry {
                name: "Second App".to_string(),
                exec: "secondapp %F".to_string(),
                desktop_file: PathBuf::from("/usr/share/applications/secondapp.desktop"),
                comment: Some("Second test application".to_string()),
                icon: None,
                is_xdg: false,
                xdg_priority: -1,
                is_default: false,
                action_id: None,
            },
        ];

        let profile = config.get_selector_profile("fzf").unwrap();

        // Generate display strings for both apps
        let mut displays = Vec::new();
        for app in &applications {
            let marker = if app.is_default {
                config.get_marker(profile, "default")
            } else if app.is_xdg {
                config.get_marker(profile, "xdg")
            } else {
                config.get_marker(profile, "available")
            };

            let comment = app
                .comment
                .as_ref()
                .map_or(String::new(), |c| format!(" - {c}"));

            let mut entry_template_engine = TemplateEngine::new();
            entry_template_engine
                .set("marker", marker)
                .set("name", &app.name)
                .set("comment", &comment);

            let display = entry_template_engine.render(&profile.entry_template);
            displays.push(display);
        }

        // Test that we can match the first app
        let selected = &displays[0];
        let mut found_index = None;
        for (i, app) in applications.iter().enumerate() {
            let marker = if app.is_default {
                config.get_marker(profile, "default")
            } else if app.is_xdg {
                config.get_marker(profile, "xdg")
            } else {
                config.get_marker(profile, "available")
            };

            let comment = app
                .comment
                .as_ref()
                .map_or(String::new(), |c| format!(" - {c}"));

            let mut entry_template_engine = TemplateEngine::new();
            entry_template_engine
                .set("marker", marker)
                .set("name", &app.name)
                .set("comment", &comment);

            let display = entry_template_engine.render(&profile.entry_template);

            if display == *selected {
                found_index = Some(i);
                break;
            }
        }

        assert_eq!(found_index, Some(0));
    }

    #[test]
    fn test_different_marker_types() {
        let config = Config::default();
        let _runner = FuzzyFinderRunner::new();
        let profile = config.get_selector_profile("fzf").unwrap();

        let default_marker = config.get_marker(profile, "default");
        let xdg_marker = config.get_marker(profile, "xdg");
        let available_marker = config.get_marker(profile, "available");

        // All markers should be different
        assert_ne!(default_marker, xdg_marker);
        assert_ne!(default_marker, available_marker);
        assert_ne!(xdg_marker, available_marker);

        // All markers should be non-empty
        assert!(!default_marker.is_empty());
        assert!(!xdg_marker.is_empty());
        assert!(!available_marker.is_empty());
    }

    #[test]
    fn test_template_substitution_in_args() {
        let config = Config::default();
        let _runner = FuzzyFinderRunner::new();
        let profile = config.get_selector_profile("fzf").unwrap();

        let mut template_engine = TemplateEngine::new();
        template_engine
            .set("file", "test.txt")
            .set("prompt", "Open 'test.txt' with: ")
            .set("header", "Available applications");

        let substituted_args = template_engine.render_args(&profile.args);

        // Should have substituted the placeholders
        let has_prompt = substituted_args.iter().any(|arg| arg.contains("test.txt"));
        assert!(has_prompt);

        let has_header = substituted_args.iter().any(|arg| arg.contains("Available"));
        assert!(has_header);
    }
}
