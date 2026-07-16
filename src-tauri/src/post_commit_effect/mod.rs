//! Closed Desktop post-commit effect boundary.

mod interface;
mod sqlite;
mod value;

pub use interface::*;
pub use sqlite::SqliteDesktopPostCommitEffectOutboxAdapterImpl;
pub(crate) use sqlite::insert_ready_post_commit_effect;
pub use value::*;
