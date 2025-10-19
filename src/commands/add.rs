use crate::cli::EditArgs;
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;

pub struct AddCommand {
    args: EditArgs,
}

impl AddCommand {
    pub fn new(args: EditArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for AddCommand {
    fn execute(self, ctx: &CommandContext) -> Result<()> {
        let mime = ctx.normalize_mime_input(&self.args.mime)?;
        ctx.ensure_handler_exists(&self.args.handler)?;

        let mut apps = ctx.load_mimeapps()?;
        apps.add_handler(&mime, self.args.handler.clone(), self.args.expand_wildcards);
        ctx.save_mimeapps(&apps)?;

        println!("Added handler {} for {}", self.args.handler, mime);
        Ok(())
    }
}
