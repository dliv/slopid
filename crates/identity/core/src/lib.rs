//! Pure decisions and values for folder identity.
pub mod allocation;
pub mod config;
pub mod folders;
pub mod lint;
pub mod list;
pub mod names;
pub mod refcode;
pub mod resolve;
pub mod slug;
pub use allocation::{NewPlan, NewResult, ScanSnapshot};
pub use config::{InitConfigResult, ProjectConfig, RootResult, render_root_human};
pub use lint::{LintVerdict, SidLintReport};
pub use list::{ListResult, ListSort, render_human};
pub use resolve::{AmbiguousCandidate, ResolveError, ResolveResult};
