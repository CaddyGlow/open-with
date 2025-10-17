use std::borrow::Cow;
use std::path::{Path, PathBuf};

use url::Url;

/// Represents the resource that should be opened by the application executor.
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchTarget {
    /// A local filesystem path that should exist on disk.
    File(PathBuf),
    /// A generic URI that should be passed verbatim to the target application.
    Uri(Url),
}

impl LaunchTarget {
    /// Returns the string that should be provided to the launched application.
    pub fn as_command_argument(&self) -> Cow<'_, str> {
        match self {
            LaunchTarget::File(path) => path.to_string_lossy(),
            LaunchTarget::Uri(uri) => Cow::Borrowed(uri.as_str()),
        }
    }

    /// A human readable label used in UI elements such as the fuzzy finder.
    pub fn display_name(&self) -> Cow<'_, str> {
        match self {
            LaunchTarget::File(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .map(Cow::Borrowed)
                .unwrap_or_else(|| path.to_string_lossy()),
            LaunchTarget::Uri(uri) => Cow::Borrowed(uri.as_str()),
        }
    }

    /// Returns the underlying path if this target represents a file.
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            LaunchTarget::File(path) => Some(path.as_path()),
            LaunchTarget::Uri(_) => None,
        }
    }
}
