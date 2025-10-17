use crate::application_finder::ApplicationEntry;
use crate::target::LaunchTarget;
use anyhow::{Context, Result};
use log::info;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct ApplicationExecutor;

impl ApplicationExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, app: &ApplicationEntry, target: &LaunchTarget) -> Result<()> {
        let prepared_command = Self::prepare_command(&app.exec, target)?;
        Self::spawn_detached(prepared_command, target)
    }

    pub fn prepare_command(exec: &str, target: &LaunchTarget) -> Result<Vec<String>> {
        let raw_parts = shell_words::split(exec)
            .map_err(|e| anyhow::anyhow!("Failed to parse exec command: {e}"))?;

        let mut parts = Vec::with_capacity(raw_parts.len());
        for part in raw_parts {
            let mut cleaned = part.replace("%%", "%");
            for placeholder in ["%u", "%U", "%f", "%F", "%i", "%c", "%k"] {
                cleaned = cleaned.replace(placeholder, "");
            }

            if cleaned.is_empty() {
                continue;
            }

            parts.push(cleaned);
        }

        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty exec command"));
        }

        // Add the file path as the last argument
        parts.push(target.as_command_argument().into_owned());

        Ok(parts)
    }

    fn spawn_detached(command_parts: Vec<String>, target: &LaunchTarget) -> Result<()> {
        if command_parts.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        info!(
            "Executing: {} \"{}\"",
            command_parts.join(" "),
            target.as_command_argument()
        );

        let mut cmd = Command::new(&command_parts[0]);

        // Add all arguments except the first (which is the command)
        for part in &command_parts[1..] {
            cmd.arg(part);
        }

        // Detach from parent process
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
}

impl Default for ApplicationExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use url::Url;

    fn create_test_application(exec: &str) -> ApplicationEntry {
        ApplicationEntry {
            name: "Test App".to_string(),
            exec: exec.to_string(),
            desktop_file: PathBuf::from("/usr/share/applications/testapp.desktop"),
            comment: Some("Test application".to_string()),
            icon: Some("testapp-icon".to_string()),
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: None,
        }
    }

    #[test]
    fn test_new_executor() {
        let _executor = ApplicationExecutor::new();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_default_executor() {
        let _executor = ApplicationExecutor::default();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_prepare_command_basic() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result = ApplicationExecutor::prepare_command("texteditor %f", &target).unwrap();

        assert_eq!(result, vec!["texteditor", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_with_args() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result = ApplicationExecutor::prepare_command("editor --readonly %f", &target).unwrap();

        assert_eq!(result, vec!["editor", "--readonly", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_clean_placeholders() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let test_cases = vec![
            ("app %f", vec!["app", "/home/user/test.txt"]),
            ("app %F", vec!["app", "/home/user/test.txt"]),
            ("app %u", vec!["app", "/home/user/test.txt"]),
            ("app %U", vec!["app", "/home/user/test.txt"]),
            ("app %i", vec!["app", "/home/user/test.txt"]),
            ("app %c", vec!["app", "/home/user/test.txt"]),
            ("app %k", vec!["app", "/home/user/test.txt"]),
            ("app %%", vec!["app", "%", "/home/user/test.txt"]),
        ];

        for (input, expected) in test_cases {
            let result = ApplicationExecutor::prepare_command(input, &target).unwrap();
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_prepare_command_multiple_placeholders() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result = ApplicationExecutor::prepare_command("app %f %u %F", &target).unwrap();

        assert_eq!(result, vec!["app", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_empty_after_cleaning() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result = ApplicationExecutor::prepare_command("   %f %F   ", &target);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty exec command");
    }

    #[test]
    fn test_prepare_command_with_whitespace() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result =
            ApplicationExecutor::prepare_command("  editor   --flag   %f  ", &target).unwrap();

        assert_eq!(result, vec!["editor", "--flag", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_complex_path() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/Documents/My File.txt"));
        let result = ApplicationExecutor::prepare_command("editor %f", &target).unwrap();

        assert_eq!(result, vec!["editor", "/home/user/Documents/My File.txt"]);
    }

    #[test]
    fn test_prepare_command_no_placeholders() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result = ApplicationExecutor::prepare_command("simple-editor", &target).unwrap();

        assert_eq!(result, vec!["simple-editor", "/home/user/test.txt"]);
    }

    #[test]
    fn test_spawn_detached_empty_command() {
        let target = LaunchTarget::File(PathBuf::from("test.txt"));
        let result = ApplicationExecutor::spawn_detached(vec![], &target);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty command");
    }

    #[test]
    fn test_execute_with_empty_exec() {
        let app = create_test_application("   %f %F   ");
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));

        let executor = ApplicationExecutor::new();
        let result = executor.execute(&app, &target);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty exec command");
    }

    #[test]
    fn test_execute_command_preparation() {
        // Test that execute properly prepares the command
        let app = create_test_application("echo %f");
        let target = LaunchTarget::File(PathBuf::from("/tmp/test.txt"));

        // We can't easily test the actual execution without side effects,
        // but we can test that the command preparation works
        let prepared = ApplicationExecutor::prepare_command(&app.exec, &target).unwrap();
        assert_eq!(prepared, vec!["echo", "/tmp/test.txt"]);
    }

    #[test]
    fn test_prepare_command_with_quotes() {
        // Test handling of commands that include quoted values
        let target = LaunchTarget::File(PathBuf::from("/home/user/test.txt"));
        let result =
            ApplicationExecutor::prepare_command("editor --title=\"My Editor\" %f", &target)
                .unwrap();

        assert_eq!(
            result,
            vec!["editor", "--title=My Editor", "/home/user/test.txt"]
        );
    }

    #[test]
    fn test_prepare_command_edge_cases() {
        let target = LaunchTarget::File(PathBuf::from("/test.txt"));

        // Test with only spaces and placeholders
        let result = ApplicationExecutor::prepare_command("   %f   %F   ", &target);
        assert!(result.is_err());

        // Test with just command name
        let result = ApplicationExecutor::prepare_command("app", &target).unwrap();
        assert_eq!(result, vec!["app", "/test.txt"]);

        // Test with escaped percent
        let result = ApplicationExecutor::prepare_command("app %%f", &target).unwrap();
        assert_eq!(result, vec!["app", "/test.txt"]);
    }

    #[test]
    fn test_prepare_command_with_uri_target() {
        let target = LaunchTarget::Uri(Url::parse("https://example.com").unwrap());
        let result = ApplicationExecutor::prepare_command("browser %u", &target).unwrap();

        // URL parser adds trailing slash for URLs without paths
        assert_eq!(result, vec!["browser", "https://example.com/"]);
    }

    #[test]
    fn test_command_parts_ordering() {
        let target = LaunchTarget::File(PathBuf::from("/home/user/document.pdf"));
        let result =
            ApplicationExecutor::prepare_command("viewer --fullscreen --page=1 %f", &target)
                .unwrap();

        assert_eq!(
            result,
            vec![
                "viewer",
                "--fullscreen",
                "--page=1",
                "/home/user/document.pdf"
            ]
        );
    }
}
