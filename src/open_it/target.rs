use crate::target::LaunchTarget;
use anyhow::{Context, Result};
use std::path::PathBuf;
use url::Url;

pub(super) fn resolve_launch_target(raw: &str) -> Result<LaunchTarget> {
    if let Ok(uri) = Url::parse(raw) {
        if uri.scheme() == "file" {
            let path = uri
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Invalid file URI: {raw}"))?;
            let path = path
                .canonicalize()
                .with_context(|| format!("Failed to resolve file path: {}", path.display()))?;
            return Ok(LaunchTarget::File(path));
        }
        return Ok(LaunchTarget::Uri(uri));
    }

    let path = PathBuf::from(raw);
    let path = path
        .canonicalize()
        .with_context(|| format!("Failed to resolve file path: {}", path.display()))?;
    Ok(LaunchTarget::File(path))
}

pub(super) fn mime_for_target(target: &LaunchTarget) -> String {
    match target {
        LaunchTarget::File(path) => {
            if path.is_dir() {
                "inode/directory".to_string()
            } else {
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string()
            }
        }
        LaunchTarget::Uri(uri) => format!("x-scheme-handler/{}", uri.scheme()),
    }
}
