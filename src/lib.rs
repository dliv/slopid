#![warn(clippy::all)]

mod agent_instructions;
mod cli;
mod inputs;
use slop_id_identity_core::{LintVerdict, render_human, render_root_human};
use slop_id_identity_fs as commands;
mod failure;
pub mod refcode;
pub mod scan;
pub mod slug;

use clap::{Parser, error::ErrorKind};
use cli::{Cli, Command};
use failure::{AmbiguousCandidate, Failure, report_failure, write_json, write_text};
use slop_id_identity_core::ResolveError;

#[expect(
    clippy::disallowed_methods,
    reason = "Process exit belongs to the CLI composition root."
)]
pub fn main_entry() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            if let Err(failure) = write_text(&error.to_string()) {
                report_failure(failure);
            }
            return;
        }
        Err(error) => report_failure(Failure::Usage(error.to_string())),
    };

    match run(cli) {
        Ok(0) => {}
        Ok(code) => std::process::exit(code),
        Err(failure) => report_failure(failure),
    }
}

fn run(cli: Cli) -> Result<i32, Failure> {
    match cli.command {
        Command::Init => {
            let cwd = current_dir()?;
            let result = commands::init_config(&cwd).map_err(Failure::Init)?;
            write_json(&result)?;
        }
        Command::Root { human } => {
            let cwd = current_dir()?;
            let result = commands::cmd_root(&cwd).map_err(Failure::Config)?;
            if human {
                write_text(&render_root_human(&result))?;
            } else {
                write_json(&result)?;
            }
        }
        Command::New {
            title,
            dry_run,
            period,
        } => {
            let cwd = current_dir()?;
            let config = commands::load_project_config(&cwd).map_err(Failure::Config)?;
            let result = commands::cmd_new(
                title,
                dry_run,
                period.unwrap_or_else(inputs::current_period),
                &config,
                inputs::randomized_tail_space,
            )
            .map_err(Failure::New)?;
            write_json(&result)?;
        }
        Command::List { terms, sort, human } => {
            let cwd = current_dir()?;
            let config = commands::load_project_config(&cwd).map_err(Failure::Config)?;
            let result = commands::cmd_list(terms, sort.into(), &config, inputs::local_timestamp)
                .map_err(Failure::List)?;
            if human {
                write_text(&render_human(&result))?;
            } else {
                write_json(&result)?;
            }
        }
        Command::Resolve { selector } => {
            let cwd = current_dir()?;
            let config = commands::load_project_config(&cwd).map_err(Failure::Config)?;
            let result = commands::cmd_resolve(&selector, &config.root).map_err(resolve_failure)?;
            write_json(&result)?;
        }
        Command::Lint => {
            let cwd = current_dir()?;
            let config = commands::load_project_config(&cwd).map_err(Failure::Config)?;
            let execution = commands::cmd_lint(&config);
            write_json(&execution)?;
            return Ok(match execution.verdict() {
                LintVerdict::Healthy => 0,
                LintVerdict::Unhealthy => 1,
                LintVerdict::Incomplete => 2,
            });
        }
        Command::AgentInstructions => {
            write_json(&agent_instructions::agent_instructions())?;
        }
    }

    Ok(0)
}

#[expect(
    clippy::disallowed_methods,
    reason = "The working directory is read at the CLI composition root."
)]
fn current_dir() -> Result<std::path::PathBuf, Failure> {
    std::env::current_dir().map_err(|error| Failure::Runtime(error.into()))
}

fn resolve_failure(error: ResolveError) -> Failure {
    match error {
        ResolveError::Invalid(message) => Failure::ResolveInvalid(message),
        ResolveError::NotFound(message) => Failure::ResolveNotFound(message),
        ResolveError::Ambiguous {
            message,
            candidates,
        } => Failure::ResolveAmbiguous {
            message,
            candidates: candidates
                .into_iter()
                .map(|candidate| AmbiguousCandidate {
                    id: candidate.id,
                    path: candidate.path,
                })
                .collect(),
        },
        ResolveError::Unavailable(error) => Failure::ResolveUnavailable(error),
    }
}
