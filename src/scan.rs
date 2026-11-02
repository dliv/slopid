//! Compatibility re-export; identity libraries own parsing and observation.
pub use slop_id_identity_core::names::{
    PeriodPrefixedChild, is_valid_period, split_task_folder_name,
};
pub use slop_id_identity_fs::scan::scan_period_prefixed_children;
