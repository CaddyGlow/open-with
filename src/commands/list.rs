use crate::cli::ListArgs;
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;
use itertools::Itertools;

pub struct ListCommand {
    args: ListArgs,
}

impl ListCommand {
    pub fn new(args: ListArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for ListCommand {
    fn execute(self, ctx: &CommandContext) -> Result<()> {
        let apps = ctx.load_mimeapps()?;

        if self.args.json {
            let payload = serde_json::json!({
                "default_apps": apps
                    .default_apps()
                    .iter()
                    .map(|(mime, handlers)| {
                        serde_json::json!({
                            "mime": mime,
                            "handlers": handlers.iter().cloned().collect::<Vec<_>>()
                        })
                    })
                    .collect::<Vec<_>>(),
                "added_associations": apps
                    .added_associations()
                    .iter()
                    .map(|(mime, handlers)| {
                        serde_json::json!({
                            "mime": mime,
                            "handlers": handlers.iter().cloned().collect::<Vec<_>>()
                        })
                    })
                    .collect::<Vec<_>>(),
            });

            println!("{}", serde_json::to_string_pretty(&payload)?);
        } else {
            for (mime, handlers) in apps.default_apps() {
                let joined = handlers.iter().map(|h| h.as_str()).join("; ");
                println!("{mime}: {joined}");
            }
        }

        Ok(())
    }
}
