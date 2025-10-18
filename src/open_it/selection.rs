use super::OpenIt;
use crate::application_finder::ApplicationEntry;
use crate::cli::SelectorKind;
use crate::config::{SelectorProfileId, SelectorProfileType};
use crate::target::LaunchTarget;
use crate::template::TemplateEngine;
use anyhow::Result;
use log::info;
use shell_words::split;
use std::io::{self, IsTerminal};

pub(super) struct LaunchContext {
    pub target: LaunchTarget,
    pub mime_type: String,
    pub applications: Vec<ApplicationEntry>,
}

impl LaunchContext {
    pub fn new(
        target: LaunchTarget,
        mime_type: String,
        applications: Vec<ApplicationEntry>,
    ) -> Self {
        Self {
            target,
            mime_type,
            applications,
        }
    }

    pub fn first_is_regex_handler(&self) -> bool {
        self.applications
            .first()
            .map(|app| {
                app.desktop_file
                    .to_string_lossy()
                    .starts_with("regex-handler-")
            })
            .unwrap_or(false)
    }
}

impl OpenIt {
    pub(super) fn launch_with_fuzzy(&self, context: &LaunchContext) -> Result<()> {
        if context.applications.len() == 1
            && (self.args.auto_open_single || context.first_is_regex_handler())
        {
            info!("Auto-opening the only available application");
            return self.execute_application(&context.applications[0], &context.target);
        }

        let display_name = context.target.display_name();
        if let Some(index) = self.run_fuzzy_finder(&context.applications, display_name.as_ref())? {
            self.execute_application(&context.applications[index], &context.target)?;
        }
        Ok(())
    }

    pub(super) fn run_selector_flow(&self, context: &LaunchContext) -> Result<()> {
        let (selector_cmd, selector_args) = self.build_selector_command(context)?;
        let log_command = if selector_args.is_empty() {
            selector_cmd.clone()
        } else {
            format!("{} {}", selector_cmd, selector_args.join(" "))
        };

        info!("Launching selector: {}", log_command);

        match self
            .selector_runner
            .run(&selector_cmd, &selector_args, &context.applications)
        {
            Ok(Some(index)) => {
                if let Some(app) = context.applications.get(index) {
                    info!(
                        "Selector chose `{}` ({})",
                        app.name,
                        app.desktop_file.display()
                    );
                }
                self.execute_application(&context.applications[index], &context.target)
            }
            Ok(None) => {
                info!("Selector produced no choice; falling back to fuzzy finder");
                self.launch_with_fuzzy(context)
            }
            Err(err) => {
                info!(
                    "Selector command failed ({}); falling back to fuzzy finder",
                    err
                );
                self.launch_with_fuzzy(context)
            }
        }
    }

    fn build_selector_command(&self, context: &LaunchContext) -> Result<(String, Vec<String>)> {
        if let Some(command_spec) = &self.args.selector_command {
            return self.selector_command_from_string(command_spec, false);
        }

        match &self.args.selector {
            SelectorKind::Auto => self.resolve_auto_selector_command(&context.target, true),
            SelectorKind::Named(name) => {
                let profile_id = SelectorProfileId::from(name.as_str());
                if let Some((cmd, args)) =
                    self.selector_command_from_profile(&profile_id, &context.target, false)?
                {
                    Ok((cmd, args))
                } else {
                    self.selector_command_from_string(name, false)
                }
            }
        }
    }

    fn run_fuzzy_finder(
        &self,
        applications: &[ApplicationEntry],
        file_name: &str,
    ) -> Result<Option<usize>> {
        let preferred_type = self.preferred_selector_profile_type();
        let profile_id = match &self.args.selector {
            SelectorKind::Auto => self
                .fuzzy_finder_runner
                .detect_available(&self.config, preferred_type)?,
            SelectorKind::Named(name) => {
                let requested = SelectorProfileId::from(name.as_str());
                if self
                    .config
                    .get_selector_profile(requested.as_ref())
                    .is_some()
                {
                    requested
                } else {
                    info!(
                        "Selector profile `{}` not found; falling back to auto fuzzy detection",
                        name
                    );
                    self.fuzzy_finder_runner
                        .detect_available(&self.config, preferred_type)?
                }
            }
        };

        self.fuzzy_finder_runner
            .run(&self.config, applications, file_name, &profile_id)
    }

    fn selector_command_from_profile(
        &self,
        profile_id: &SelectorProfileId,
        target: &LaunchTarget,
        append_term_args: bool,
    ) -> Result<Option<(String, Vec<String>)>> {
        let profile = if let Some(profile) = self.config.get_selector_profile(profile_id.as_ref()) {
            profile
        } else {
            return Ok(None);
        };

        let display_name = target.display_name();
        let mut template_engine = TemplateEngine::new();
        template_engine.set("file", display_name.as_ref());
        let prompt = template_engine.render(self.config.get_prompt_template(profile));
        let header = template_engine.render(self.config.get_header_template(profile));
        template_engine
            .set("prompt", &prompt)
            .set("header", &header);
        let mut args = template_engine.render_args(&profile.args);

        if append_term_args {
            if let Some(extra) = &self.config.selector.term_exec_args {
                if !extra.trim().is_empty() {
                    let extra_parts = split(extra)
                        .map_err(|e| anyhow::anyhow!("Failed to parse selector exec args: {e}"))?;
                    args.extend(extra_parts);
                }
            }
        }

        Ok(Some((profile.command.clone(), args)))
    }

    fn selector_command_from_string(
        &self,
        command_spec: &str,
        append_term_args: bool,
    ) -> Result<(String, Vec<String>)> {
        let mut parts = split(command_spec)
            .map_err(|e| anyhow::anyhow!("Failed to parse selector command: {e}"))?;
        if parts.is_empty() {
            anyhow::bail!("Selector command is empty");
        }

        let command = parts.remove(0);

        if append_term_args {
            if let Some(extra) = &self.config.selector.term_exec_args {
                if !extra.trim().is_empty() {
                    let extra_parts = split(extra)
                        .map_err(|e| anyhow::anyhow!("Failed to parse selector exec args: {e}"))?;
                    parts.extend(extra_parts);
                }
            }
        }

        Ok((command, parts))
    }

    fn preferred_selector_profile_type(&self) -> SelectorProfileType {
        if io::stdout().is_terminal() {
            SelectorProfileType::Tui
        } else {
            SelectorProfileType::Gui
        }
    }

    fn selector_name_candidates(&self) -> Vec<SelectorProfileId> {
        self.config
            .selector_candidates(self.preferred_selector_profile_type())
    }

    fn resolve_auto_selector_command(
        &self,
        target: &LaunchTarget,
        append_term_args: bool,
    ) -> Result<(String, Vec<String>)> {
        let candidates = self.selector_name_candidates();
        let mut last_error: Option<anyhow::Error> = None;

        for name in candidates {
            match self.selector_command_from_profile(&name, target, append_term_args)? {
                Some(result) => return Ok(result),
                None => match self.selector_command_from_string(name.as_ref(), append_term_args) {
                    Ok(result) => return Ok(result),
                    Err(err) => last_error = Some(err),
                },
            }
        }

        if let Some(err) = last_error {
            Err(err)
        } else {
            anyhow::bail!("No selector command configured for auto mode")
        }
    }
}
