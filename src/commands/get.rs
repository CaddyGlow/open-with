use crate::application_finder::ApplicationFinder;
use crate::cli::GetArgs;
use crate::commands::{CommandContext, CommandExecutor};
use anyhow::Result;
use std::collections::{BTreeMap, HashSet};
use wildmatch::WildMatch;

pub struct GetCommand {
    args: GetArgs,
}

impl GetCommand {
    pub fn new(args: GetArgs) -> Self {
        Self { args }
    }
}

impl CommandExecutor for GetCommand {
    fn execute(self, ctx: &CommandContext) -> Result<()> {
        let pattern = ctx.normalize_mime_input(&self.args.mime)?;
        let finder = ctx.application_finder();

        if pattern.contains('*') {
            handle_wildcard_query(&finder, &pattern, &self.args)?;
        } else {
            handle_exact_query(&finder, &pattern, &self.args)?;
        }

        Ok(())
    }
}

fn handle_wildcard_query(finder: &ApplicationFinder, pattern: &str, args: &GetArgs) -> Result<()> {
    let all_mime_types: HashSet<String> = finder.all_mime_types().into_iter().collect();
    let matcher = WildMatch::new(pattern);
    let matching_mimes: Vec<String> = all_mime_types
        .into_iter()
        .filter(|mime| matcher.matches(mime))
        .collect();

    if args.json {
        let mut results = BTreeMap::new();
        for mime in &matching_mimes {
            let applications = finder.find_for_mime(mime, args.actions);
            if !applications.is_empty() {
                results.insert(mime.clone(), applications);
            }
        }
        let output = serde_json::json!({
            "pattern": pattern,
            "matching_mimes": matching_mimes,
            "results": results,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Pattern: {}", pattern);
        println!("Matching MIME types: {}", matching_mimes.len());

        if matching_mimes.is_empty() {
            println!("No MIME types match this pattern.");
        } else {
            for mime in matching_mimes {
                let applications = finder.find_for_mime(&mime, args.actions);
                if applications.is_empty() {
                    continue;
                }

                println!("\n{} ({} applications):", mime, applications.len());
                for app in &applications {
                    let mut prefix = "  ";
                    if app.is_default {
                        prefix = "★ ";
                    } else if app.is_xdg {
                        prefix = "▶ ";
                    }
                    print!("  {}{}", prefix, app.name);
                    if let Some(action_id) = &app.action_id {
                        print!(" [action: {}]", action_id);
                    }
                    println!();
                }
            }
            println!("\nLegend: ★=Default  ▶=XDG Associated  (space)=Available");
        }
    }

    Ok(())
}

fn handle_exact_query(finder: &ApplicationFinder, pattern: &str, args: &GetArgs) -> Result<()> {
    let applications = finder.find_for_mime(pattern, args.actions);

    if args.json {
        let xdg_associations: Vec<String> = vec![];
        let output = serde_json::json!({
            "mimetype": pattern,
            "xdg_associations": xdg_associations,
            "applications": applications,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("MIME type: {}", pattern);
        if applications.is_empty() {
            println!("No applications found for this MIME type.");
        } else {
            println!("\nAvailable applications ({}):", applications.len());
            for (index, app) in applications.iter().enumerate() {
                let mut prefix = "  ";
                if app.is_default {
                    prefix = "★ ";
                } else if app.is_xdg {
                    prefix = "▶ ";
                }

                print!("{}{}", prefix, app.name);
                if let Some(action_id) = &app.action_id {
                    print!(" [action: {}]", action_id);
                }
                println!();

                if let Some(comment) = &app.comment {
                    println!("    {}", comment);
                }
                println!("    Exec: {}", app.exec);
                println!("    Desktop file: {}", app.desktop_file.display());

                if index < applications.len() - 1 {
                    println!();
                }
            }
            println!("\nLegend: ★=Default  ▶=XDG Associated  (space)=Available");
        }
    }

    Ok(())
}
