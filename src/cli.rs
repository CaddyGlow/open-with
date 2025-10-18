use clap::{Args as ClapArgs, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorKind {
    Auto,
    Named(String),
}

impl SelectorKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("Selector value cannot be empty".to_string());
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower == "auto" {
            Ok(SelectorKind::Auto)
        } else if lower == "fzf" {
            Ok(SelectorKind::Named("fzf".to_string()))
        } else if lower == "fuzzel" {
            Ok(SelectorKind::Named("fuzzel".to_string()))
        } else {
            Ok(SelectorKind::Named(trimmed.to_string()))
        }
    }
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
    #[command(subcommand)]
    pub command: Command,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct OpenArgs {
    /// Resource to open; accepts filesystem paths or URIs.
    pub target: Option<String>,

    /// Selector profile to use
    #[arg(long, default_value = "auto", value_parser = SelectorKind::parse, alias = "fuzzer")]
    pub selector: SelectorKind,

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

    /// Override selector enablement (true/false)
    #[arg(long)]
    pub enable_selector: Option<bool>,

    /// Override selector command (e.g. `rofi -dmenu`)
    #[arg(long = "selector-command")]
    pub selector_command: Option<String>,

    /// Override terminal exec args passed to selector commands
    #[arg(long = "term-exec-args")]
    pub term_exec_args: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Open a resource using the configured handlers.
    Open(OpenArgs),
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
    /// Get available applications for a MIME type or extension.
    Get(GetArgs),
    /// Generate a shell completion script.
    Completions(CompletionsArgs),
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

#[derive(ClapArgs, Debug, Clone)]
pub struct GetArgs {
    /// MIME type or file extension to query.
    #[arg(value_name = "MIME_OR_EXT")]
    pub mime: String,
    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
    /// Show desktop actions as separate entries.
    #[arg(short, long)]
    pub actions: bool,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct CompletionsArgs {
    /// Target shell for the completions (bash, zsh, fish, powershell, elvish, fig, nushell).
    #[arg(value_enum)]
    pub shell: Shell,
    /// Optional output path (prints to stdout when not provided).
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Override the binary name used in the generated script.
    #[arg(long, default_value = "openit")]
    pub bin_name: String,
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
    fn test_cli_open_subcommand() {
        let cli = Cli::try_parse_from(["openit", "open", "file.txt"]).unwrap();
        match cli.command {
            Command::Open(open) => {
                assert_eq!(open.target.as_deref(), Some("file.txt"));
                assert_eq!(open.selector, SelectorKind::Auto);
            }
            other => panic!("Expected open command, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_named_selector_profile() {
        let cli =
            Cli::try_parse_from(["openit", "open", "--selector", "rofi", "file.txt"]).unwrap();
        match cli.command {
            Command::Open(open) => {
                assert_eq!(open.selector, SelectorKind::Named("rofi".to_string()));
            }
            other => panic!("Expected open command, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_set_subcommand() {
        let cli = Cli::try_parse_from(["openit", "set", "text/plain", "helix.desktop"]).unwrap();
        match cli.command {
            Command::Set(args) => {
                assert_eq!(args.mime, "text/plain");
                assert_eq!(args.handler, "helix.desktop");
                assert!(!args.expand_wildcards);
            }
            _ => panic!("Expected set command"),
        }
    }

    #[test]
    fn test_cli_requires_subcommand() {
        let cli = Cli::try_parse_from(["openit"]);
        assert!(cli.is_err(), "CLI should require an explicit subcommand");
    }

    #[test]
    fn test_cli_parse_help() {
        Cli::command().debug_assert();
    }
}
