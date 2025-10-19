use crate::cli;
use crate::cli::OpenArgs;
use crate::commands::{CommandContext, CommandExecutor};
use crate::config;
use crate::open_it::OpenIt;
use anyhow::Result;
use std::fs;

pub struct OpenCommand {
    args: OpenArgs,
}

impl OpenCommand {
    pub fn new(args: OpenArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for OpenCommand {
    fn execute(self, _ctx: &CommandContext) -> Result<()> {
        let args = self.args;

        if let Err(message) = args.validate() {
            anyhow::bail!(message);
        }

        if args.build_info {
            cli::show_build_info();
            return Ok(());
        }

        if args.generate_config {
            generate_config(&args)?;
            return Ok(());
        }

        let level = match args.verbose {
            0 => "warn",
            1 => "info",
            _ => "debug",
        };

        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

        let app = OpenIt::new(args)?;
        app.run()
    }
}

fn generate_config(args: &OpenArgs) -> Result<()> {
    let config = config::Config::default();
    if let Some(custom_path) = &args.config {
        if let Some(parent) = custom_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml_string = toml::to_string_pretty(&config)?;
        fs::write(custom_path, toml_string)?;
        println!(
            "Generated default configuration at: {}",
            custom_path.display()
        );
    } else {
        config.save()?;
        println!(
            "Generated default configuration at: {}",
            config::Config::config_path().display()
        );
    }
    Ok(())
}
