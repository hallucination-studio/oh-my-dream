//! Closed Desktop post-commit effect boundary.

mod interface;
mod sqlite;
mod value;

pub use interface::*;
pub use sqlite::SqliteDesktopPostCommitEffectOutboxAdapterImpl;
pub use value::*;
