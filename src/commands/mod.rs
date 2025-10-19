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
pub use mime::normalize_mime_input;
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
