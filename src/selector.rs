use crate::application_finder::ApplicationEntry;
use anyhow::{Context, Result};
use itertools::Itertools;
use log::info;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Default)]
pub struct SelectorRunner;

impl SelectorRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run(
        &self,
        command: &str,
        args: &[String],
        applications: &[ApplicationEntry],
    ) -> Result<Option<usize>> {
        if applications.is_empty() {
            return Ok(None);
        }

        let command_spec = command.trim();
        if command_spec.is_empty() {
            return Err(anyhow::anyhow!("Selector command is empty"));
        }

        let mut cmd = Command::new(command);
        for arg in args {
            cmd.arg(arg);
        }

        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn selector command `{}`", command))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Selector command `{}` has no stdin", command))?;

        for app in applications {
            let marker = marker_for_app(app);
            writeln!(stdin, "{marker} {}", app.name)?;
        }

        let output = child
            .wait_with_output()
            .context("Failed to read selector output")?;

        if !output.status.success() {
            info!(
                "Selector command `{}` exited with status {:?}",
                command,
                output.status.code()
            );
            return Ok(None);
        }

        let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selection.is_empty() {
            info!("Selector command `{}` returned no selection", command);
            return Ok(None);
        }

        let selection_cleaned = strip_marker(&selection);

        let index = applications
            .iter()
            .position(|app| app.name == selection_cleaned)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Selector returned unknown selection `{selection}` (expected one of [{}])",
                    applications.iter().map(|app| app.name.as_str()).join(", ")
                )
            })?;

        Ok(Some(index))
    }
}

fn marker_for_app(app: &ApplicationEntry) -> &'static str {
    if app.desktop_file.starts_with("regex-handler-") {
        "[regex]"
    } else if app.is_default {
        "[default]"
    } else if app.is_xdg {
        "[xdg]"
    } else {
        "[available]"
    }
}

fn strip_marker(selection: &str) -> String {
    if let Some(rest) = selection.strip_prefix('[') {
        if let Some(idx) = rest.find(']') {
            return rest[idx + 1..].trim_start().to_string();
        }
    }
    selection.to_string()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn test_app(name: &str) -> ApplicationEntry {
        ApplicationEntry {
            name: name.to_string(),
            exec: format!("{name} %F"),
            desktop_file: std::path::PathBuf::from(format!("{name}.desktop")),
            comment: None,
            icon: None,
            is_xdg: false,
            xdg_priority: -1,
            is_default: false,
            action_id: None,
        }
    }

    fn create_script(contents: &str) -> (TempDir, String) {
        let dir = TempDir::new().unwrap();
        let script_path = dir.path().join("script.sh");
        fs::write(&script_path, contents).unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
        (dir, script_path.to_string_lossy().to_string())
    }

    #[test]
    fn test_selector_runner_selects_second_entry() {
        let script = r#"#!/bin/sh
read first
read second
printf "%s" "$second"
"#;

        let (_dir, script_path) = create_script(script);

        let runner = SelectorRunner::new();
        let command = "sh";
        let args = vec![script_path.clone()];

        let mut apps = vec![test_app("First"), test_app("Second")];
        apps[1].desktop_file = std::path::PathBuf::from("regex-handler-1.desktop");

        let index = runner.run(command, &args, &apps).unwrap();
        assert_eq!(index, Some(1));
    }

    #[test]
    fn test_selector_runner_handles_cancellation() {
        let script = r#"#!/bin/sh
# Exit without printing a selection to simulate cancellation
exit 0
"#;

        let (_dir, script_path) = create_script(script);

        let runner = SelectorRunner::new();
        let command = "sh";
        let args = vec![script_path.clone()];

        let apps = vec![test_app("Only")];

        let index = runner.run(command, &args, &apps).unwrap();
        assert_eq!(index, None);
    }

    #[test]
    fn test_selector_runner_appends_term_exec_args() {
        let dir = TempDir::new().unwrap();
        let output_path = dir.path().join("args.txt");

        let script = format!(
            r#"#!/bin/sh
echo "$@" > {}
read choice
printf "%s" "$choice"
"#,
            output_path.display()
        );

        let (_dir, script_path) = create_script(&script);

        let runner = SelectorRunner::new();
        let command = "sh";
        let args = vec![script_path.clone(), "--flag".into(), "value".into()];

        let apps = vec![test_app("Only")];

        let index = runner.run(command, &args, &apps).unwrap();
        assert_eq!(index, Some(0));

        let args_contents = fs::read_to_string(output_path).unwrap();
        assert_eq!(args_contents.trim(), "--flag value");
    }

    #[test]
    fn test_selector_runner_rejects_unknown_selection() {
        let script = r#"#!/bin/sh
printf "Unknown"
"#;

        let (_dir, script_path) = create_script(script);

        let runner = SelectorRunner::new();
        let command = "sh";
        let args = vec![script_path.clone()];

        let apps = vec![test_app("One")];

        let err = runner.run(command, &args, &apps).unwrap_err();
        assert!(err
            .to_string()
            .contains("Selector returned unknown selection"));
    }
}
