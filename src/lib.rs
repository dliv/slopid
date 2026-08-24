#![warn(clippy::all)]

mod cli;
mod commands;
mod documents;
mod input;
mod markdown;
pub mod refcode;
pub mod scan;
pub mod slug;
mod walk;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use serde::Serialize;

pub fn main_entry() {
    let cli = Cli::parse();

    match run(cli) {
        Ok(0) => {}
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {:#}", err);
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::New {
            title,
            from_seed,
            dry_run,
            period,
            into,
        } => {
            let cwd = std::env::current_dir()?;
            let inputs = commands::NewInputs {
                title,
                from_seed,
                dry_run,
                period,
                into,
            };
            let result = commands::cmd_new(inputs, &cwd)?;
            output(&result)?;
        }
        Command::List { terms, sort, human } => {
            let cwd = std::env::current_dir()?;
            let result = commands::cmd_list(terms, sort, &cwd)?;
            if human {
                print!("{}", commands::render_human(&result));
            } else {
                output(&result)?;
            }
        }
        Command::Resolve { id } => {
            let cwd = std::env::current_dir()?;
            output(&commands::cmd_resolve(&id, &cwd)?)?;
        }
        Command::Graph {
            id,
            depth,
            direction,
            edges,
        } => {
            let cwd = std::env::current_dir()?;
            output(&commands::cmd_graph(&id, depth, direction, &edges, &cwd)?)?;
        }
        Command::Lint => {
            let cwd = std::env::current_dir()?;
            let execution = commands::cmd_lint(&cwd)?;
            if let Some(result) = execution.result {
                output(&result)?;
            } else {
                eprintln!(
                    "error: cannot complete lint scan because a configured root or entrypoint is unreadable"
                );
            }
            return Ok(execution.exit_code);
        }
        Command::Search { terms, limit } => {
            let cwd = std::env::current_dir()?;
            output(&commands::cmd_search(terms, limit, &cwd)?)?;
        }
        Command::Context { id, depth } => {
            let cwd = std::env::current_dir()?;
            output(&commands::cmd_context(&id, depth, &cwd)?)?;
        }
        Command::Captures => {
            let cwd = std::env::current_dir()?;
            output(&commands::cmd_captures(&cwd)?)?;
        }
        Command::Note { text } => {
            let cwd = std::env::current_dir()?;
            let input = input::collect_note(text)?;
            output(&commands::cmd_note(input, &cwd)?)?;
        }
        Command::Seed {
            title,
            origins,
            edit,
            dry_run,
        } => {
            let cwd = std::env::current_dir()?;
            let body = if dry_run {
                String::new()
            } else {
                input::collect_seed_body(edit)?
            };
            output(&commands::cmd_seed(
                commands::SeedInputs {
                    title,
                    origins,
                    dry_run,
                    body,
                },
                &cwd,
            )?)?;
        }
        Command::Relink {
            write,
            move_id,
            into,
            expected_plan_sha256,
        } => {
            let cwd = std::env::current_dir()?;
            let inputs = commands::RelinkInputs {
                write,
                move_id,
                into,
                expected_plan_sha256,
            };
            output(&commands::cmd_relink(inputs, &cwd)?)?;
        }
        Command::AgentInstructions => {
            let result = commands::agent_instructions();
            output(&result)?;
        }
        Command::Init => {
            let cwd = std::env::current_dir()?;
            let result = commands::init_config(&cwd)?;
            output(&result)?;
        }
    }

    Ok(0)
}

fn output<T: Serialize>(result: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(result)?);
    Ok(())
}
