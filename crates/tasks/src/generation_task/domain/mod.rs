//! The Generation Task aggregate and its immutable domain values.

mod aggregate;
mod canonical;
mod error;
mod failure;
mod identity;
mod request;
mod result;
mod state;

pub use aggregate::*;
pub use error::*;
pub use failure::*;
pub use identity::*;
pub use request::*;
pub use result::*;
pub use state::*;
