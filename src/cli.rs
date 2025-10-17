use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
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
    long_about = None,
    subcommand_precedence_over_arg = true
)]
pub struct Cli {
    #[command(flatten)]
    pub open: OpenArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct OpenArgs {
    /// Resource to open; accepts filesystem paths or URIs.
    pub target: Option<String>,

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

    /// Generate default configuration file
    #[arg(long)]
    pub generate_config: bool,

    /// Path to configuration file
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Automatically open if only one application is available (skip picker)
    #[arg(long)]
    pub auto_open_single: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Set the default handler for a MIME type or extension.
    Set(EditArgs),
    /// Add an additional handler (after the default) for a MIME type or extension.
    Add(EditArgs),
    /// Remove a handler from a MIME type or extension.
    Remove(RemoveArgs),
    /// Unset the default handlers for a MIME type or extension.
    Unset(UnsetArgs),
    /// List configured handlers.
    List(ListArgs),
}

#[derive(ClapArgs, Debug, Clone)]
pub struct EditArgs {
    /// MIME type or file extension to update.
    #[arg(value_name = "MIME_OR_EXT")]
    pub mime: String,
    /// Desktop file to apply (e.g. `code.desktop`).
    #[arg(value_name = "HANDLER")]
    pub handler: String,
    /// Expand wildcard MIME patterns to the currently known concrete MIME keys.
    #[arg(long)]
    pub expand_wildcards: bool,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct RemoveArgs {
    /// MIME type or file extension to update.
    #[arg(value_name = "MIME_OR_EXT")]
    pub mime: String,
    /// Desktop file to remove (e.g. `code.desktop`).
    #[arg(value_name = "HANDLER")]
    pub handler: String,
    /// Expand wildcard MIME patterns to the currently known concrete MIME keys.
    #[arg(long)]
    pub expand_wildcards: bool,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct UnsetArgs {
    /// MIME type or file extension to update.
    #[arg(value_name = "MIME_OR_EXT")]
    pub mime: String,
    /// Expand wildcard MIME patterns to the currently known concrete MIME keys.
    #[arg(long)]
    pub expand_wildcards: bool,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct ListArgs {
    /// Output handler info as JSON.
    #[arg(long)]
    pub json: bool,
}

impl OpenArgs {
    /// Validate arguments and return errors for invalid combinations.
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), String> {
        if !self.build_info && !self.clear_cache && !self.generate_config && self.target.is_none() {
            return Err("A target argument is required unless using --build-info, --clear-cache, or --generate-config".to_string());
        }
        Ok(())
    }

    /// Get the provided target (file path or URI) as a borrowed string.
    #[allow(dead_code)]
    pub fn get_target(&self) -> Option<&str> {
        self.target.as_deref()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_default_open() {
        let cli = Cli::try_parse_from(["open-with", "file.txt"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.open.target.as_deref(), Some("file.txt"));
        assert_eq!(cli.open.fuzzer, FuzzyFinder::Auto);
    }

    #[test]
    fn test_cli_set_subcommand() {
        let cli = Cli::try_parse_from(["open-with", "set", "text/plain", "helix.desktop"]).unwrap();
        match cli.command {
            Some(Command::Set(args)) => {
                assert_eq!(args.mime, "text/plain");
                assert_eq!(args.handler, "helix.desktop");
                assert!(!args.expand_wildcards);
            }
            _ => panic!("Expected set command"),
        }
    }

    #[test]
    fn test_cli_parse_help() {
        Cli::command().debug_assert();
    }
}
