use crate::cli::EditArgs;
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;

pub struct SetCommand {
    args: EditArgs,
}

impl SetCommand {
    pub fn new(args: EditArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for SetCommand {
    fn execute(self, ctx: &CommandContext) -> Result<()> {
        let mime = ctx.normalize_mime_input(&self.args.mime)?;
        ctx.ensure_handler_exists(&self.args.handler)?;

        let mut apps = ctx.load_mimeapps()?;
        apps.set_handler(
            &mime,
            vec![self.args.handler.clone()],
            self.args.expand_wildcards,
        );
        ctx.save_mimeapps(&apps)?;

        println!("Set default handler for {mime} -> {}", self.args.handler);
        Ok(())
    }
}
