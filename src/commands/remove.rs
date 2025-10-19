use crate::cli::RemoveArgs;
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;

pub struct RemoveCommand {
    args: RemoveArgs,
}

impl RemoveCommand {
    pub fn new(args: RemoveArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for RemoveCommand {
    fn execute(self, ctx: &CommandContext) -> Result<()> {
        let mime = ctx.normalize_mime_input(&self.args.mime)?;

        let mut apps = ctx.load_mimeapps()?;
        apps.remove_handler(
            &mime,
            Some(self.args.handler.as_str()),
            self.args.expand_wildcards,
        );
        ctx.save_mimeapps(&apps)?;

        println!("Removed handler {} from {}", self.args.handler, mime);
        Ok(())
    }
}
