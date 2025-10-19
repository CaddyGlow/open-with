use crate::cli::Command;
use anyhow::Result;

mod add;
mod completions;
mod context;
mod get;
mod list;
mod mime;
mod open;
mod remove;
mod set;
mod unset;

pub use add::AddCommand;
pub use completions::CompletionsCommand;
pub use context::CommandContext;
pub use get::GetCommand;
pub use list::ListCommand;
pub use open::OpenCommand;
pub use remove::RemoveCommand;
pub use set::SetCommand;
pub use unset::UnsetCommand;

pub trait CommandExecutor {
    fn execute(self, ctx: &CommandContext) -> Result<()>;
}

pub fn dispatch(command: Command) -> Result<()> {
    let ctx = CommandContext::default();

    match command {
        Command::Open(args) => OpenCommand::new(args).execute(&ctx),
        Command::Set(args) => SetCommand::new(args).execute(&ctx),
        Command::Add(args) => AddCommand::new(args).execute(&ctx),
        Command::Remove(args) => RemoveCommand::new(args).execute(&ctx),
        Command::Unset(args) => UnsetCommand::new(args).execute(&ctx),
        Command::List(args) => ListCommand::new(args).execute(&ctx),
        Command::Get(args) => GetCommand::new(args).execute(&ctx),
        Command::Completions(args) => CompletionsCommand::new(args).execute(&ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Command, EditArgs, RemoveArgs, UnsetArgs};
    use crate::test_support::{ConfigEnvGuard, ValidationEnvGuard};
    use serial_test::serial;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn dispatch_set_add_unset() {
        let temp_config = TempDir::new().unwrap();
        let _config_guard = ConfigEnvGuard::set(temp_config.path());
        let _validation = ValidationEnvGuard::enable();

        dispatch(Command::Set(EditArgs {
            mime: "text/plain".into(),
            handler: "helix.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let config_path = temp_config.path().join("mimeapps.list");
        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.contains("text/plain=helix.desktop;"));

        dispatch(Command::Add(EditArgs {
            mime: "text/plain".into(),
            handler: "code.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.contains("text/plain=helix.desktop;code.desktop;"));

        dispatch(Command::Unset(UnsetArgs {
            mime: "text/plain".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.trim().is_empty());
    }

    #[test]
    #[serial]
    fn dispatch_remove_handler() {
        let temp_config = TempDir::new().unwrap();
        let _config_guard = ConfigEnvGuard::set(temp_config.path());
        let _validation = ValidationEnvGuard::enable();

        dispatch(Command::Set(EditArgs {
            mime: "text/plain".into(),
            handler: "helix.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        dispatch(Command::Add(EditArgs {
            mime: "text/plain".into(),
            handler: "code.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        dispatch(Command::Remove(RemoveArgs {
            mime: "text/plain".into(),
            handler: "helix.desktop".into(),
            expand_wildcards: false,
        }))
        .unwrap();

        let contents = fs::read_to_string(temp_config.path().join("mimeapps.list")).unwrap();
        assert!(contents.contains("text/plain=code.desktop;"));
        assert!(!contents.contains("helix.desktop"));
    }

    #[test]
    #[serial]
    fn dispatch_add_missing_handler_errors() {
        env::remove_var(ValidationEnvGuard::KEY);

        let temp_config = TempDir::new().unwrap();
        let _config_guard = ConfigEnvGuard::set(temp_config.path());

        let result = dispatch(Command::Add(EditArgs {
            mime: "text/plain".into(),
            handler: "nonexistent.desktop".into(),
            expand_wildcards: false,
        }));

        assert!(result.is_err());
        let message = format!("{}", result.unwrap_err());
        assert!(message.contains("Desktop handler"));
    }
}
