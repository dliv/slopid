use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sid",
    version = env!("CARGO_PKG_VERSION"),
    about = "Use a deterministic local STM protocol with JSON results",
    after_help = "Read/query: root, list, resolve, graph, lint, search, context, captures\nControlled mutation: new, note, seed, relink --write\nSafe default: relink previews; inspect complete, applied, changes, and findings\n\nFor AI agent usage instructions: sid agent-instructions"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Print the configured active task root
    Root {
        /// Plain path output for direct human use instead of JSON
        /// (agents should parse the default JSON)
        #[arg(long)]
        human: bool,
    },

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
    #[command(after_help = "Global relink repairs every resolvable destination.\n\
Projected relink instead scopes repairs to the destinations that one planned\n\
lifecycle move would change:\n\
  sid relink --move sa2a7 --into .archive\n\
  sid relink --move sa2a7 --into .archive --write --expected-plan-sha256 <SHA256>\n\
Both forms preview by default. A projected write requires the exact digest from\n\
a fresh preview and refuses before touching any file when the plan changed or\n\
is incomplete; a partial result requires another preview. Projected complete\n\
means move-caused repair authority is safe, not that every local destination\n\
resolves. A generic destination proven absent under both current and projected\n\
readings yields relink-unresolved-local-destination with severity:warning. Its\n\
finding and complete source bytes bind the digest, but it creates no replacement\n\
or completeness failure; exact-digest warning-only write is zero-change\n\
convergence. Ambiguity, unknown inspection, unsafe representation, and actual\n\
move-caused drift remain errors. Settled verification retains warnings. Excluded\n\
inbox, note, tmp, VCS, and ignored paths remain outside this proof boundary.\n\
\n\
A projected --write and the owner rename you perform afterwards need a\n\
quiescent authored-source window: keep editors and background agents from\n\
writing the scanned Markdown until both are done. Slopid does not detect or\n\
lease writers. Its per-file byte check skips a file whose bytes already\n\
changed before the check; it is not a lock, lease, or compare-and-swap, so a\n\
write landing after that check and before atomic replacement is overwritten.")]
    Relink {
        /// Apply independently verified per-file repairs; default is preview
        #[arg(long)]
        write: bool,

        /// Scope repairs to one exact task/review id that is about to move
        #[arg(long = "move", value_name = "ID", requires = "into")]
        move_id: Option<String>,

        /// Configured task root the moving owner is projected into,
        /// e.g. --into .archive or --into stm/.archive-prs
        #[arg(long, value_name = "ROOT", requires = "move_id")]
        into: Option<String>,

        /// Exact `plan_sha256` from a fresh projected preview; required to
        /// write a projected move
        #[arg(
            long,
            value_name = "SHA256",
            requires_all = ["write", "move_id"],
            value_parser = parse_sha256
        )]
        expected_plan_sha256: Option<String>,
    },

    /// Print AI agent usage instructions
    AgentInstructions,

    /// Write the default project config to .sid
    Init,
}

/// A plan digest is exactly the wire form `plan_sha256` emits: 64 lowercase
/// ASCII hexadecimal characters. Anything else is an argument error, so an
/// approval can never be silently truncated or case-folded into a match.
fn parse_sha256(value: &str) -> Result<String, String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(value.to_string())
    } else {
        Err("value must be exactly 64 lowercase hexadecimal characters".to_string())
    }
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
