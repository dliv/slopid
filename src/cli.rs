use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sid",
    version = env!("CARGO_PKG_VERSION"),
    about = "Allocate and identify local task folders",
    after_help = "Identity commands: init, root, new, list, resolve, lint\n\nFor AI agent usage instructions: sid agent-instructions"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Write the default project config to .sid
    Init,

    /// Print the project base and configured identity root
    Root {
        /// Print only the root path for direct human use
        #[arg(long)]
        human: bool,
    },

    /// Allocate a new task folder id
    New {
        /// Human task title, normalized into the id slug
        title: String,

        /// Describe the folder that would be created without creating it
        #[arg(long)]
        dry_run: bool,

        /// Override the YYYYMM period (hidden test seam; shape-checked only)
        #[arg(long, hide = true)]
        period: Option<String>,
    },

    /// List direct identity folders, most recently touched first
    List {
        /// Search terms: each must match the ref as a prefix or the slug as a
        /// substring (case-insensitive)
        #[arg(value_name = "TERM")]
        terms: Vec<String>,

        /// Sort order
        #[arg(long, value_enum, default_value_t = crate::commands::ListSort::Recent)]
        sort: crate::commands::ListSort,

        /// Plain column output for direct human use
        #[arg(long)]
        human: bool,
    },

    /// Resolve an exact short ref or complete folder name
    Resolve {
        /// Exact, case-sensitive short ref or identity folder name
        selector: String,
    },

    /// Audit direct-child folder identity; exits 0 healthy, 1 unhealthy, 2 incomplete
    Lint,

    /// Print AI agent usage instructions
    AgentInstructions,
}
