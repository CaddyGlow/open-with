use super::OpenIt;
use crate::application_finder::{ApplicationEntry, ApplicationSource};
use crate::executor::ApplicationExecutor;
use crate::regex_handlers::RegexHandler;
use crate::target::LaunchTarget;
use anyhow::{Context, Result};
use log::info;
use std::path::PathBuf;

impl OpenIt {
    pub(super) fn execute_application(
        &self,
        app: &ApplicationEntry,
        target: &LaunchTarget,
    ) -> Result<()> {
        if app.requires_terminal {
            let launcher = self.resolve_terminal_launcher()?;
            self.executor
                .execute(app, target, Some(launcher.as_slice()))
        } else {
            self.executor.execute(app, target, None)
        }
    }

    pub(crate) fn resolve_terminal_launcher(&self) -> Result<Vec<String>> {
        let mut candidates = self
            .application_finder
            .find_for_mime("x-scheme-handler/terminal", false);

        if candidates.is_empty() {
            candidates = self.application_finder.find_terminal_emulators();
        }

        if candidates.is_empty() {
            anyhow::bail!(
                "No terminal emulator found. Install a terminal or associate one with x-scheme-handler/terminal."
            );
        }

        let terminal_app = candidates
            .iter()
            .find(|app| !app.requires_terminal)
            .or_else(|| candidates.first())
            .ok_or_else(|| anyhow::anyhow!("No suitable terminal emulator found"))?;

        info!(
            "Using terminal emulator `{}` ({})",
            terminal_app.name,
            terminal_app.desktop_file.display()
        );

        ApplicationExecutor::base_command_parts(&terminal_app.exec).with_context(|| {
            format!(
                "Failed to prepare terminal command from `{}`",
                terminal_app.exec
            )
        })
    }
}

pub(super) fn application_from_regex(handler: &RegexHandler) -> ApplicationEntry {
    let patterns = handler.patterns().join(", ");
    let name = handler
        .notes
        .clone()
        .unwrap_or_else(|| format!("Regex handler (prio {})", handler.priority));

    let comment = if patterns.is_empty() {
        format!("Regex handler -> {}", handler.exec)
    } else {
        format!("Regex handler -> {} [{patterns}]", handler.exec)
    };

    ApplicationEntry {
        name,
        exec: handler.exec.clone(),
        desktop_file: PathBuf::from(format!("regex-handler-{}.desktop", handler.priority)),
        comment: Some(comment),
        icon: None,
        is_xdg: false,
        xdg_priority: handler.priority,
        is_default: false,
        action_id: None,
        requires_terminal: handler.terminal,
        is_terminal_emulator: false,
    }
    .with_source(ApplicationSource::Regex {
        priority: handler.priority,
    })
}
