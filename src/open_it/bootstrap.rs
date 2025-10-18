use crate::cache::{DesktopCache, FileSystemCache};
use crate::cli::OpenArgs;
use crate::config;
use crate::desktop_parser::DesktopFile;
use anyhow::{Context, Result};
use log::{debug, info};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use walkdir::WalkDir;

pub(super) struct BootstrapOutcome {
    pub desktop_cache: Box<dyn DesktopCache>,
    pub config: config::Config,
}

pub(super) fn initialize(args: &OpenArgs) -> Result<BootstrapOutcome> {
    let config = config::Config::load(args.config.clone()).with_context(|| {
        args.config
            .as_ref()
            .map(|path| format!("Failed to load configuration from {}", path.display()))
            .unwrap_or_else(|| "Failed to load configuration".to_string())
    })?;

    Ok(BootstrapOutcome {
        desktop_cache: load_desktop_cache(),
        config,
    })
}

pub(crate) fn clear_cache() -> Result<()> {
    let cache_path = cache_path();
    if cache_path.exists() {
        match fs::remove_file(&cache_path) {
            Ok(()) => info!("Cache cleared"),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                info!("No cache to clear");
            }
            Err(e) => return Err(e).context("Failed to remove cache file"),
        }
    } else {
        info!("No cache to clear");
    }
    Ok(())
}

pub(crate) fn load_desktop_cache() -> Box<dyn DesktopCache> {
    let cache_path = cache_path();
    let mut cache = FileSystemCache::new(cache_path);

    if let Err(e) = cache.load() {
        debug!("Failed to load cache: {e}");
    }

    let desktop_dirs = crate::xdg::get_desktop_file_paths();
    let mut cache_updated = false;
    let rebuild = cache.needs_invalidation() || cache.is_empty();

    if rebuild {
        debug!("Building desktop file cache");
        cache.clear();
        cache_updated |= populate_cache_from_dirs(&mut cache, &desktop_dirs, true);
    } else {
        debug!("Loaded desktop cache from disk");
        cache_updated |= populate_cache_from_dirs(&mut cache, &desktop_dirs, false);
    }

    if rebuild || cache_updated {
        if let Err(e) = cache.save() {
            debug!("Failed to save cache: {e}");
        }
    }

    Box::new(cache)
}

pub(crate) fn populate_cache_from_dirs(
    cache: &mut FileSystemCache,
    desktop_dirs: &[PathBuf],
    force: bool,
) -> bool {
    let mut updated = false;

    for dir in desktop_dirs {
        if !dir.exists() {
            debug!("Directory does not exist: {}", dir.display());
            continue;
        }

        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| !s.starts_with('.'))
                    .unwrap_or(false)
            })
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }

            let already_cached = if force {
                false
            } else {
                DesktopCache::get(&*cache, path).is_some()
            };

            if already_cached {
                continue;
            }

            match DesktopFile::parse(path) {
                Ok(desktop_file) => {
                    DesktopCache::insert(cache, path.to_path_buf(), desktop_file);
                    updated = true;
                }
                Err(e) => {
                    debug!("Failed to parse {}: {}", path.display(), e);
                }
            }
        }
    }

    updated
}

pub(crate) fn cache_path() -> PathBuf {
    if let Ok(override_path) = env::var("OPEN_WITH_CACHE_PATH") {
        return PathBuf::from(override_path);
    }

    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("openit")
        .join("desktop_cache.json")
}
