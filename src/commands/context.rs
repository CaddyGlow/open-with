use crate::application_finder::ApplicationFinder;
use crate::mime_associations::MimeAssociations;
use crate::mimeapps::MimeApps;
use crate::open_it::OpenIt;
use anyhow::Result;
use std::path::Path;

pub const SKIP_HANDLER_VALIDATION_ENV: &str = "OPEN_WITH_SKIP_HANDLER_VALIDATION";

#[derive(Debug, Default)]
pub struct CommandContext;

impl CommandContext {
    pub fn normalize_mime_input(&self, input: &str) -> Result<String> {
        super::mime::normalize_mime_input(input)
    }

    pub fn load_mimeapps(&self) -> Result<MimeApps> {
        MimeApps::load_from_disk(None)
    }

    pub fn save_mimeapps(&self, apps: &MimeApps) -> Result<()> {
        apps.save_to_disk(None)
    }

    pub fn ensure_handler_exists(&self, handler: &str) -> Result<()> {
        ensure_handler_exists(handler)
    }

    pub fn application_finder(&self) -> ApplicationFinder {
        ApplicationFinder::new(OpenIt::load_desktop_cache(), MimeAssociations::load())
    }
}

fn ensure_handler_exists(handler: &str) -> Result<()> {
    if should_skip_handler_validation() {
        return Ok(());
    }

    if handler.trim().is_empty() {
        anyhow::bail!("Handler identifier cannot be empty");
    }

    let path = Path::new(handler);
    if (path.is_absolute() || handler.contains('/')) && path.exists() {
        return Ok(());
    }

    let cache = OpenIt::load_desktop_cache();
    let finder = ApplicationFinder::new(cache, MimeAssociations::default());

    if finder.find_desktop_file(handler).is_none() {
        anyhow::bail!(
            "Desktop handler `{}` not found in available applications",
            handler
        );
    }

    Ok(())
}

fn should_skip_handler_validation() -> bool {
    cfg!(test) && std::env::var(SKIP_HANDLER_VALIDATION_ENV).is_ok()
}
