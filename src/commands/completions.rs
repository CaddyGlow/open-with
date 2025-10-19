use crate::cli::{Cli, CompletionsArgs};
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;
use clap::CommandFactory;
use std::fs;

pub struct CompletionsCommand {
    args: CompletionsArgs,
}

impl CompletionsCommand {
    pub fn new(args: CompletionsArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for CompletionsCommand {
    fn execute(self, _ctx: &CommandContext) -> Result<()> {
        let mut command = Cli::command();
        let shell = self.args.shell;
        let bin_name = self.args.bin_name;

        if let Some(path) = self.args.output {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(&path)?;
            clap_complete::generate(shell, &mut command, bin_name.clone(), &mut file);
            println!("Generated {shell} completions at {}", path.display());
        } else {
            let mut stdout = std::io::stdout();
            clap_complete::generate(shell, &mut command, bin_name, &mut stdout);
        }

        Ok(())
    }
}
