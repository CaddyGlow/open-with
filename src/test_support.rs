#![cfg(test)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CacheEnvGuard {
    original: Option<OsString>,
}

impl CacheEnvGuard {
    const KEY: &'static str = "OPEN_WITH_CACHE_PATH";

    pub fn set(path: &Path) -> Self {
        let original = env::var_os(Self::KEY);
        env::set_var(Self::KEY, path);
        Self { original }
    }
}

impl Drop for CacheEnvGuard {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            env::set_var(Self::KEY, original);
        } else {
            env::remove_var(Self::KEY);
        }
    }
}

pub struct ConfigEnvGuard {
    original: Option<OsString>,
}

impl ConfigEnvGuard {
    const KEY: &'static str = "XDG_CONFIG_HOME";

    pub fn set(path: &Path) -> Self {
        let original = env::var_os(Self::KEY);
        env::set_var(Self::KEY, path);
        Self { original }
    }
}

impl Drop for ConfigEnvGuard {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            env::set_var(Self::KEY, original);
        } else {
            env::remove_var(Self::KEY);
        }
    }
}

pub struct ValidationEnvGuard {
    original: Option<OsString>,
}

impl ValidationEnvGuard {
    pub const KEY: &'static str = "OPEN_WITH_SKIP_HANDLER_VALIDATION";

    pub fn enable() -> Self {
        let original = env::var_os(Self::KEY);
        env::set_var(Self::KEY, "1");
        Self { original }
    }
}

impl Drop for ValidationEnvGuard {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            env::set_var(Self::KEY, original);
        } else {
            env::remove_var(Self::KEY);
        }
    }
}

pub fn create_test_desktop_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let file_path = dir.join(name);
    fs::write(&file_path, content).expect("failed to write desktop file");
    file_path
}
