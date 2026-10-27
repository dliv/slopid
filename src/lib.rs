#![warn(clippy::all)]

mod cli;
mod commands;
mod failure;
pub mod refcode;
pub mod scan;
pub mod slug;

use clap::{Parser, error::ErrorKind};
use cli::{Cli, Command};
use commands::ResolveError;
use failure::{AmbiguousCandidate, Failure, report_failure, write_json, write_text};

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
                write_text(&commands::render_root_human(&result))?;
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
            let result =
                commands::cmd_new(title, dry_run, period, &config).map_err(Failure::New)?;
            write_json(&result)?;
        }
        Command::List { terms, sort, human } => {
            let cwd = current_dir()?;
            let config = commands::load_project_config(&cwd).map_err(Failure::Config)?;
            let result = commands::cmd_list(terms, sort, &config).map_err(Failure::List)?;
            if human {
                write_text(&commands::render_human(&result))?;
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
            write_json(&execution.report)?;
            return Ok(execution.exit_code);
        }
        Command::AgentInstructions => {
            write_json(&commands::agent_instructions())?;
        }
    }

    Ok(0)
}

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
