use crate::application_finder::{ApplicationEntry, ApplicationFinder};
use crate::cache::DesktopCache;
#[cfg(test)]
use crate::cache::FileSystemCache;
use crate::cli::OpenArgs;
use crate::config;
use crate::executor::ApplicationExecutor;
use crate::fuzzy_finder::FuzzyFinderRunner;
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
    pub(crate) fuzzy_finder_runner: FuzzyFinderRunner,
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

        if let Some(enable_selector) = args.enable_selector {
            config.selector.enable_selector = enable_selector;
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
            fuzzy_finder_runner: FuzzyFinderRunner::new(),
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

        if !self.config.selector.enable_selector {
            if context.first_is_regex_handler() {
                info!("Selector disabled; launching regex handler directly");
                return self.execute_application(&context.applications[0], &context.target);
            }
            return self.launch_with_fuzzy(&context);
        }

        if self.args.json || !io::stdout().is_terminal() {
            return self.output_json(&context);
        }

        if context.applications.len() == 1 && self.args.auto_open_single {
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
            if !path.is_file() {
                anyhow::bail!("Path is not a file: {}", path.display());
            }
            info!("File: {}", path.display());
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
