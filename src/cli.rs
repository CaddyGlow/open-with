use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum FuzzyFinder {
    Fzf,
    Fuzzel,
    Auto,
}

#[derive(Parser, Debug)]
#[command(
    author = "Your Name",
    version = crate::built_info::PKG_VERSION,
    about = "Enhanced file opener with XDG MIME support",
    long_about = None
)]
pub struct Args {
    /// File to open (not required when using --build-info or --clear-cache)
    pub file: Option<PathBuf>,

    /// Fuzzy finder to use
    #[arg(long, value_enum, default_value = "auto")]
    pub fuzzer: FuzzyFinder,

    /// Output JSON instead of interactive mode
    #[arg(short, long)]
    pub json: bool,

    /// Show desktop actions as separate entries
    #[arg(short, long)]
    pub actions: bool,

    /// Clear the desktop file cache
    #[arg(long)]
    pub clear_cache: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Show build information
    #[arg(long)]
    pub build_info: bool,
}

impl Args {
    /// Validate arguments and return errors for invalid combinations
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), String> {
        if !self.build_info && !self.clear_cache && self.file.is_none() {
            return Err(
                "File argument is required unless using --build-info or --clear-cache".to_string(),
            );
        }
        Ok(())
    }

    /// Get the file path, ensuring it exists when needed
    #[allow(dead_code)]
    pub fn get_file(&self) -> Option<&PathBuf> {
        self.file.as_ref()
    }
}

pub fn show_build_info() {
    println!("Version: {}", crate::built_info::PKG_VERSION);

    // Build time is available as a string constant
    println!("Built: {}", crate::built_info::BUILT_TIME_UTC);

    if let Some(hash) = crate::built_info::GIT_COMMIT_HASH {
        println!("Commit: {hash}");
    } else {
        println!("Commit: unknown");
    }

    if let Some(hash_short) = crate::built_info::GIT_COMMIT_HASH_SHORT {
        println!("Commit (short): {hash_short}");
    }

    if let Some(branch) = crate::built_info::GIT_HEAD_REF {
        println!("Branch: {branch}");
    } else {
        println!("Branch: unknown");
    }

    println!("Target: {}", crate::built_info::TARGET);
    println!("Rustc: {}", crate::built_info::RUSTC_VERSION);

    if let Some(dirty) = crate::built_info::GIT_DIRTY {
        if dirty {
            println!("Git status: dirty (uncommitted changes)");
        } else {
            println!("Git status: clean");
        }
    } else {
        println!("Git status: unknown");
    }

    // Additional useful build info
    println!("Profile: {}", crate::built_info::PROFILE);

    // if let Some(features) = crate::built_info::FEATURES_STR.as_deref() {
    //     if !features.is_empty() {
    //         println!("Features: {}", features);
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_args_parsing_with_file() {
        // Test basic file argument
        let args = Args::try_parse_from(["open-with", "test.txt"]).unwrap();
        assert_eq!(args.file, Some(PathBuf::from("test.txt")));
        assert_eq!(args.fuzzer, FuzzyFinder::Auto);
        assert!(!args.json);
        assert!(!args.actions);
        assert!(!args.clear_cache);
        assert!(!args.verbose);
        assert!(!args.build_info);
    }

    #[test]
    fn test_args_parsing_build_info_only() {
        // Test --build-info without file
        let args = Args::try_parse_from(["open-with", "--build-info"]).unwrap();
        assert_eq!(args.file, None);
        assert!(args.build_info);

        // Should pass validation
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_args_validation_missing_file() {
        // Test missing file without --build-info or --clear-cache should fail validation
        let args = Args::try_parse_from(["open-with"]).unwrap();
        assert_eq!(args.file, None);
        assert!(!args.build_info);
        assert!(!args.clear_cache);

        // Should fail validation
        assert!(args.validate().is_err());
    }

    #[test]
    fn test_args_validation_clear_cache_only() {
        // Test --clear-cache without file should pass validation
        let args = Args::try_parse_from(["open-with", "--clear-cache"]).unwrap();
        assert_eq!(args.file, None);
        assert!(args.clear_cache);

        // Should pass validation
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_args_with_all_flags() {
        let args = Args::try_parse_from([
            "open-with",
            "test.txt",
            "--fuzzer",
            "fzf",
            "--json",
            "--actions",
            "--clear-cache",
            "--verbose",
            "--build-info",
        ])
        .unwrap();

        assert_eq!(args.file, Some(PathBuf::from("test.txt")));
        assert_eq!(args.fuzzer, FuzzyFinder::Fzf);
        assert!(args.json);
        assert!(args.actions);
        assert!(args.clear_cache);
        assert!(args.verbose);
        assert!(args.build_info);
    }

    #[test]
    fn test_fuzzy_finder_enum_values() {
        use clap::ValueEnum;
        assert_eq!(
            FuzzyFinder::from_str("fzf", false).unwrap(),
            FuzzyFinder::Fzf
        );
        assert_eq!(
            FuzzyFinder::from_str("fuzzel", false).unwrap(),
            FuzzyFinder::Fuzzel
        );
        assert_eq!(
            FuzzyFinder::from_str("auto", false).unwrap(),
            FuzzyFinder::Auto
        );
    }

    #[test]
    fn test_short_flags() {
        let args = Args::try_parse_from([
            "open-with",
            "test.txt",
            "-j", // --json
            "-a", // --actions
            "-v", // --verbose
        ])
        .unwrap();

        assert!(args.json);
        assert!(args.actions);
        assert!(args.verbose);
    }

    #[test]
    fn test_get_file_method() {
        let args_with_file = Args::try_parse_from(["open-with", "test.txt"]).unwrap();
        assert_eq!(args_with_file.get_file(), Some(&PathBuf::from("test.txt")));

        let args_without_file = Args::try_parse_from(["open-with", "--build-info"]).unwrap();
        assert_eq!(args_without_file.get_file(), None);
    }

    #[test]
    fn test_command_structure() {
        // Verify the command structure is valid
        Args::command().debug_assert();
    }

    #[test]
    fn test_help_text_generation() {
        let mut cmd = Args::command();
        let help = cmd.render_help();

        // Verify help contains key elements
        let help_str = help.to_string();
        assert!(help_str.contains("Enhanced file opener with XDG MIME support"));
        assert!(help_str.contains("--fuzzer"));
        assert!(help_str.contains("--json"));
        assert!(help_str.contains("--actions"));
        assert!(help_str.contains("--build-info"));
    }

    #[test]
    fn test_version_info() {
        let cmd = Args::command();
        let version = cmd.get_version().unwrap();

        // Version should be set from built info
        assert!(!version.is_empty());
        assert_eq!(version, crate::built_info::PKG_VERSION);
    }

    #[test]
    fn test_fuzzer_default_value() {
        let args = Args::try_parse_from(["open-with", "test.txt"]).unwrap();
        assert_eq!(args.fuzzer, FuzzyFinder::Auto);
    }

    #[test]
    fn test_fuzzer_explicit_value() {
        let args = Args::try_parse_from(["open-with", "test.txt", "--fuzzer", "fzf"]).unwrap();
        assert_eq!(args.fuzzer, FuzzyFinder::Fzf);
    }

    #[test]
    fn test_invalid_fuzzer_value() {
        let result = Args::try_parse_from(["open-with", "test.txt", "--fuzzer", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_info_without_other_args() {
        let args = Args::try_parse_from(["open-with", "--build-info"]).unwrap();
        assert!(args.build_info);
        assert_eq!(args.file, None);
        assert_eq!(args.fuzzer, FuzzyFinder::Auto); // Should still have default
    }

    #[test]
    fn test_show_build_info() {
        // Capture stdout to test the function
        use std::io::Cursor;
        use std::sync::Mutex;
        
        // This test just ensures the function runs without panicking
        // We can't easily test the output without mocking stdout
        show_build_info();
        
        // Verify that build constants exist and are accessible
        assert!(!crate::built_info::PKG_VERSION.is_empty());
        assert!(!crate::built_info::TARGET.is_empty());
        assert!(!crate::built_info::RUSTC_VERSION.is_empty());
        assert!(!crate::built_info::PROFILE.is_empty());
    }
}
