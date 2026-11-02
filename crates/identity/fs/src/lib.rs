//! Real filesystem workflows for folder identity.
mod allocation;
mod config;
mod folders;
mod lint;
mod list;
mod resolve;
pub mod scan;
pub use allocation::cmd_new;
pub use config::{cmd_root, init_config, load_project_config};
pub use lint::cmd_lint;
pub use list::cmd_list;
pub use resolve::cmd_resolve;
