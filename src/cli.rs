use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sid",
    version = env!("CARGO_PKG_VERSION"),
    about = "Use a deterministic local STM protocol with JSON results",
    after_help = "Read/query: list, resolve, graph, lint, search, context, captures\nControlled mutation: new, note, seed, relink --write\nSafe default: relink previews; inspect complete, applied, changes, and findings\n\nFor AI agent usage instructions: sid agent-instructions"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Allocate a new task folder id
    New {
        /// Human task title, normalized into the id slug
        #[arg(required_unless_present = "from_seed")]
        title: Option<String>,

        /// Graduate one exact parked seed id into a task folder
        #[arg(long, value_name = "ID", conflicts_with = "title")]
        from_seed: Option<String>,

        /// Describe the folder that would be created without creating it
        #[arg(long)]
        dry_run: bool,

        /// Override the YYYYMM period (hidden test seam; shape-checked only,
        /// calendar validity stays deferred)
        #[arg(long, hide = true, conflicts_with = "from_seed")]
        period: Option<String>,

        /// Allocate into a configured root instead of the active root,
        /// e.g. --into .slow or --into stm/.slow
        #[arg(long, value_name = "ROOT")]
        into: Option<String>,
    },

    /// List task folders and reservations across the configured roots,
    /// most recently touched first
    List {
        /// Search terms: each must match the ref as a prefix or the slug as
        /// a substring (case-insensitive)
        #[arg(value_name = "TERM")]
        terms: Vec<String>,

        /// Sort order
        #[arg(long, value_enum, default_value_t = crate::commands::ListSort::Recent)]
        sort: crate::commands::ListSort,

        /// Plain column output for direct human use instead of JSON
        /// (formatting is not a stable interface; agents should parse the
        /// default JSON)
        #[arg(long)]
        human: bool,
    },

    /// Resolve one exact canonical STM id to its entrypoint and frontmatter JSON
    Resolve {
        /// Exact, case-sensitive frontmatter id
        id: String,
    },

    /// Read the connected STM relationship component as deterministic JSON
    Graph {
        /// Exact, case-sensitive anchor frontmatter id
        id: String,

        /// Maximum shortest-hop distance from the anchor
        #[arg(long)]
        depth: Option<usize>,

        /// Direction for origin and supersedes edges
        #[arg(long, value_enum, default_value_t = crate::commands::GraphDirection::Both)]
        direction: crate::commands::GraphDirection,

        /// Relationship type to include; repeat to select more than one
        #[arg(long = "edge", value_enum)]
        edges: Vec<crate::commands::GraphEdgeType>,
    },

    /// Audit STM frontmatter and relationship integrity as stable JSON
    Lint,

    /// Search authored text and captures as deterministic JSON
    Search {
        /// Case-insensitive literal terms; every term must match
        #[arg(value_name = "TERM", num_args = 1..)]
        terms: Vec<String>,

        /// Maximum owner results to return; totals remain uncapped
        #[arg(long, default_value_t = 20, value_parser = parse_positive_usize)]
        limit: usize,
    },

    /// Read one task/review graph plus its pending inbox as JSON
    Context {
        /// Exact folder-backed task or review id
        id: String,
    },

    /// Inventory pending notes and parked seeds as JSON without note bodies
    Captures,

    /// Capture one identity-free note and return its state/path as JSON
    Note {
        /// Capture text; when omitted, read non-TTY stdin or open an editor
        text: Option<String>,
    },

    /// Create or preview one parked seed and return allocation JSON
    Seed {
        /// Seed title
        title: String,

        /// Exact canonical origin id; repeat to preserve multiple origins
        #[arg(long = "origin")]
        origins: Vec<String>,

        /// Open the editor chain for an optional body
        #[arg(long)]
        edit: bool,

        /// Validate and describe the seed without reading input or writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Preview or apply safe Markdown destination repairs as JSON
    Relink {
        /// Apply independently verified per-file repairs; default is preview
        #[arg(long)]
        write: bool,
    },

    /// Print AI agent usage instructions
    AgentInstructions,

    /// Write the default project config to .sid
    Init,
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "value must be a positive integer".to_string())?;
    if parsed == 0 {
        Err("value must be positive".to_string())
    } else {
        Ok(parsed)
    }
}
