/// Result structs and implementations for identity commands. Commands return
/// data; the CLI shell chooses output channels and exit semantics.
mod agent_instructions;
mod config;
mod folders;
mod lint;
mod list;
mod new;
mod resolve;
mod root;

pub use agent_instructions::*;
pub use config::*;
pub use lint::*;
pub use list::*;
pub use new::*;
pub use resolve::*;
pub use root::*;
