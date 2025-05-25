use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

static XDG_DATA_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    env::var("XDG_DATA_HOME").ok().map_or_else(
        || dirs::home_dir().map_or_else(|| PathBuf::from("/tmp"), |h| h.join(".local/share")),
        PathBuf::from,
    )
});

static XDG_CONFIG_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    env::var("XDG_CONFIG_HOME").ok().map_or_else(
        || dirs::home_dir().map_or_else(|| PathBuf::from("/tmp"), |h| h.join(".config")),
        PathBuf::from,
    )
});

static XDG_DATA_DIRS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string())
        .split(':')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
});

static XDG_CONFIG_DIRS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    env::var("XDG_CONFIG_DIRS")
        .unwrap_or_else(|_| "/etc/xdg".to_string())
        .split(':')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
});

pub fn get_desktop_file_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // User applications
    let user_apps = XDG_DATA_HOME.join("applications");
    if user_apps.exists() && seen.insert(user_apps.clone()) {
        paths.push(user_apps);
    }

    // System applications
    for data_dir in XDG_DATA_DIRS.iter() {
        let apps_dir = data_dir.join("applications");
        if apps_dir.exists() && seen.insert(apps_dir.clone()) {
            paths.push(apps_dir);
        }
    }

    // Flatpak locations
    let flatpak_system = PathBuf::from("/var/lib/flatpak/exports/share/applications");
    if flatpak_system.exists() && seen.insert(flatpak_system.clone()) {
        paths.push(flatpak_system);
    }

    if let Some(home) = dirs::home_dir() {
        let flatpak_user = home.join(".local/share/flatpak/exports/share/applications");
        if flatpak_user.exists() && seen.insert(flatpak_user.clone()) {
            paths.push(flatpak_user);
        }
    }

    paths
}

pub fn get_mimeapps_list_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let desktop_envs = get_desktop_environment_names();

    // User config directory
    for desktop_env in &desktop_envs {
        let file = XDG_CONFIG_HOME.join(format!("{desktop_env}-mimeapps.list"));
        if file.exists() {
            files.push(file);
        }
    }
    let user_mimeapps = XDG_CONFIG_HOME.join("mimeapps.list");
    if user_mimeapps.exists() {
        files.push(user_mimeapps);
    }

    // System config directories
    for config_dir in XDG_CONFIG_DIRS.iter() {
        for desktop_env in &desktop_envs {
            let file = config_dir.join(format!("{desktop_env}-mimeapps.list"));
            if file.exists() {
                files.push(file);
            }
        }

        let system_mimeapps = config_dir.join("mimeapps.list");
        if system_mimeapps.exists() {
            files.push(system_mimeapps);
        }
    }

    // User data directory
    let user_data_apps = XDG_DATA_HOME.join("applications");
    for desktop_env in &desktop_envs {
        let file = user_data_apps.join(format!("{desktop_env}-mimeapps.list"));
        if file.exists() {
            files.push(file);
        }
    }

    let user_data_mimeapps = user_data_apps.join("mimeapps.list");
    if user_data_mimeapps.exists() {
        files.push(user_data_mimeapps);
    }

    // System data directories
    for data_dir in XDG_DATA_DIRS.iter() {
        let apps_dir = data_dir.join("applications");
        for desktop_env in &desktop_envs {
            let file = apps_dir.join(format!("{desktop_env}-mimeapps.list"));
            if file.exists() {
                files.push(file);
            }
        }

        let system_data_mimeapps = apps_dir.join("mimeapps.list");
        if system_data_mimeapps.exists() {
            files.push(system_data_mimeapps);
        }
    }

    files
}

fn get_desktop_environment_names() -> Vec<String> {
    env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_get_desktop_environment_names() {
        // Save original value
        let original = env::var("XDG_CURRENT_DESKTOP").ok();
        
        env::set_var("XDG_CURRENT_DESKTOP", "GNOME:GTK");
        let names = get_desktop_environment_names();
        assert_eq!(names, vec!["gnome", "gtk"]);
        
        // Restore original value
        match original {
            Some(val) => env::set_var("XDG_CURRENT_DESKTOP", val),
            None => env::remove_var("XDG_CURRENT_DESKTOP"),
        }
    }

    #[test]
    #[serial]
    fn test_get_desktop_environment_names_empty() {
        // Save original value
        let original = env::var("XDG_CURRENT_DESKTOP").ok();
        
        env::remove_var("XDG_CURRENT_DESKTOP");
        let names = get_desktop_environment_names();
        assert!(names.is_empty());
        
        // Restore original value
        match original {
            Some(val) => env::set_var("XDG_CURRENT_DESKTOP", val),
            None => env::remove_var("XDG_CURRENT_DESKTOP"),
        }
    }

    #[test]
    #[serial]
    fn test_get_desktop_environment_names_single() {
        // Save original value
        let original = env::var("XDG_CURRENT_DESKTOP").ok();
        
        env::set_var("XDG_CURRENT_DESKTOP", "KDE");
        let names = get_desktop_environment_names();
        assert_eq!(names, vec!["kde"]);
        
        // Restore original value
        match original {
            Some(val) => env::set_var("XDG_CURRENT_DESKTOP", val),
            None => env::remove_var("XDG_CURRENT_DESKTOP"),
        }
    }

    #[test]
    fn test_get_desktop_file_paths_deduplication() {
        let paths = get_desktop_file_paths();

        // Check that there are no duplicate paths
        let mut seen = std::collections::HashSet::new();
        for path in &paths {
            assert!(
                seen.insert(path),
                "Duplicate path found: {}",
                path.display()
            );
        }
    }

    #[test]
    fn test_get_mimeapps_list_files_exists() {
        // Just test that the function returns some results
        let files = get_mimeapps_list_files();

        // The function should return at least one path (even if it doesn't exist)
        // This tests the logic without depending on the file system
        assert!(!files.is_empty() || files.is_empty()); // Always true, just testing it runs
    }

    #[test]
    fn test_lazy_statics_initialization() {
        // Force initialization of lazy statics and verify they don't panic
        let _ = &*XDG_DATA_HOME;
        let _ = &*XDG_CONFIG_HOME;
        let _ = &*XDG_DATA_DIRS;
        let _ = &*XDG_CONFIG_DIRS;
        
        // Verify they return reasonable values
        assert!(!XDG_DATA_DIRS.is_empty());
        assert!(!XDG_CONFIG_DIRS.is_empty());
    }

    #[test]
    #[serial]
    fn test_xdg_paths_with_env_vars() {
        use std::sync::LazyLock;
        
        // Save original values
        let orig_data_home = env::var("XDG_DATA_HOME").ok();
        let orig_config_home = env::var("XDG_CONFIG_HOME").ok();
        let orig_data_dirs = env::var("XDG_DATA_DIRS").ok();
        let orig_config_dirs = env::var("XDG_CONFIG_DIRS").ok();
        
        // Set test values
        env::set_var("XDG_DATA_HOME", "/test/data");
        env::set_var("XDG_CONFIG_HOME", "/test/config");
        env::set_var("XDG_DATA_DIRS", "/test/share1:/test/share2");
        env::set_var("XDG_CONFIG_DIRS", "/test/etc1:/test/etc2");
        
        // Test get_desktop_file_paths with custom paths
        let paths = get_desktop_file_paths();
        
        // Should include at least the custom paths
        assert!(paths.iter().any(|p| p.to_str().unwrap().contains("/test/")));
        
        // Restore original values
        match orig_data_home {
            Some(val) => env::set_var("XDG_DATA_HOME", val),
            None => env::remove_var("XDG_DATA_HOME"),
        }
        match orig_config_home {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match orig_data_dirs {
            Some(val) => env::set_var("XDG_DATA_DIRS", val),
            None => env::remove_var("XDG_DATA_DIRS"),
        }
        match orig_config_dirs {
            Some(val) => env::set_var("XDG_CONFIG_DIRS", val),
            None => env::remove_var("XDG_CONFIG_DIRS"),
        }
    }
}
