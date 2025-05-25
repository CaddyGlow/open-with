use crate::application_finder::ApplicationEntry;
use anyhow::{Context, Result};
use log::info;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct ApplicationExecutor;

impl ApplicationExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(app: &ApplicationEntry, file_path: &Path) -> Result<()> {
        let prepared_command = Self::prepare_command(&app.exec, file_path)?;
        Self::spawn_detached(prepared_command, file_path)
    }

    pub fn prepare_command(exec: &str, file_path: &Path) -> Result<Vec<String>> {
        let clean_exec = exec
            .replace("%%", "%") // Handle escaped % first
            .replace("%u", "")
            .replace("%U", "")
            .replace("%f", "")
            .replace("%F", "")
            .replace("%i", "")
            .replace("%c", "")
            .replace("%k", "")
            .trim()
            .to_string();

        if clean_exec.is_empty() {
            return Err(anyhow::anyhow!("Empty exec command"));
        }

        let mut parts: Vec<String> = clean_exec
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect();

        // Add the file path as the last argument
        parts.push(file_path.to_string_lossy().to_string());

        Ok(parts)
    }

    fn spawn_detached(command_parts: Vec<String>, file_path: &Path) -> Result<()> {
        if command_parts.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        info!(
            "Executing: {} \"{}\"",
            command_parts.join(" "),
            file_path.display()
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

    fn create_test_application(exec: &str) -> ApplicationEntry {
        crate::application_finder::ApplicationEntryBuilder::new()
            .name("Test App")
            .exec(exec)
            .desktop_file("/usr/share/applications/testapp.desktop")
            .comment("Test application")
            .icon("testapp-icon")
            .as_available()
            .build()
            .unwrap()
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
        let file_path = Path::new("/home/user/test.txt");
        let result = ApplicationExecutor::prepare_command("texteditor %f", file_path).unwrap();

        assert_eq!(result, vec!["texteditor", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_with_args() {
        let file_path = Path::new("/home/user/test.txt");
        let result =
            ApplicationExecutor::prepare_command("editor --readonly %f", file_path).unwrap();

        assert_eq!(result, vec!["editor", "--readonly", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_clean_placeholders() {
        let file_path = Path::new("/home/user/test.txt");
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
            let result = ApplicationExecutor::prepare_command(input, file_path).unwrap();
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_prepare_command_multiple_placeholders() {
        let file_path = Path::new("/home/user/test.txt");
        let result = ApplicationExecutor::prepare_command("app %f %u %F", file_path).unwrap();

        assert_eq!(result, vec!["app", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_empty_after_cleaning() {
        let file_path = Path::new("/home/user/test.txt");
        let result = ApplicationExecutor::prepare_command("   %f %F   ", file_path);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty exec command");
    }

    #[test]
    fn test_prepare_command_with_whitespace() {
        let file_path = Path::new("/home/user/test.txt");
        let result =
            ApplicationExecutor::prepare_command("  editor   --flag   %f  ", file_path).unwrap();

        assert_eq!(result, vec!["editor", "--flag", "/home/user/test.txt"]);
    }

    #[test]
    fn test_prepare_command_complex_path() {
        let file_path = Path::new("/home/user/Documents/My File.txt");
        let result = ApplicationExecutor::prepare_command("editor %f", file_path).unwrap();

        assert_eq!(result, vec!["editor", "/home/user/Documents/My File.txt"]);
    }

    #[test]
    fn test_prepare_command_no_placeholders() {
        let file_path = Path::new("/home/user/test.txt");
        let result = ApplicationExecutor::prepare_command("simple-editor", file_path).unwrap();

        assert_eq!(result, vec!["simple-editor", "/home/user/test.txt"]);
    }

    #[test]
    fn test_spawn_detached_empty_command() {
        let result = ApplicationExecutor::spawn_detached(vec![], Path::new("test.txt"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty command");
    }

    #[test]
    fn test_execute_with_empty_exec() {
        let app = create_test_application("   %f %F   ");
        let file_path = Path::new("/home/user/test.txt");

        let result = ApplicationExecutor::execute(&app, file_path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Empty exec command");
    }

    #[test]
    fn test_execute_command_preparation() {
        // Test that execute properly prepares the command
        let app = create_test_application("echo %f");
        let file_path = Path::new("/tmp/test.txt");

        // We can't easily test the actual execution without side effects,
        // but we can test that the command preparation works
        let prepared = ApplicationExecutor::prepare_command(&app.exec, file_path).unwrap();
        assert_eq!(prepared, vec!["echo", "/tmp/test.txt"]);
    }

    #[test]
    fn test_prepare_command_with_quotes() {
        // Test handling of commands that might have quotes (though our current
        // implementation doesn't handle shell quoting)
        let file_path = Path::new("/home/user/test.txt");
        let result =
            ApplicationExecutor::prepare_command("editor --title=\"My Editor\" %f", file_path)
                .unwrap();

        assert_eq!(
            result,
            vec!["editor", "--title=\"My", "Editor\"", "/home/user/test.txt"]
        );
    }

    #[test]
    fn test_prepare_command_edge_cases() {
        let file_path = Path::new("/test.txt");

        // Test with only spaces and placeholders
        let result = ApplicationExecutor::prepare_command("   %f   %F   ", file_path);
        assert!(result.is_err());

        // Test with just command name
        let result = ApplicationExecutor::prepare_command("app", file_path).unwrap();
        assert_eq!(result, vec!["app", "/test.txt"]);

        // Test with escaped percent
        let result = ApplicationExecutor::prepare_command("app %%f", file_path).unwrap();
        assert_eq!(result, vec!["app", "/test.txt"]);
    }

    #[test]
    fn test_command_parts_ordering() {
        let file_path = Path::new("/home/user/document.pdf");
        let result =
            ApplicationExecutor::prepare_command("viewer --fullscreen --page=1 %f", file_path)
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
