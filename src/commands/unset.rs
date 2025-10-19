use crate::cli::UnsetArgs;
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;

pub struct UnsetCommand {
    args: UnsetArgs,
}

impl UnsetCommand {
    pub fn new(args: UnsetArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for UnsetCommand {
    fn execute(self, ctx: &CommandContext) -> Result<()> {
        let mime = ctx.normalize_mime_input(&self.args.mime)?;

        let mut apps = ctx.load_mimeapps()?;
        apps.remove_handler(&mime, None, self.args.expand_wildcards);
        ctx.save_mimeapps(&apps)?;

        println!("Unset handlers for {}", mime);
        Ok(())
    }
}
