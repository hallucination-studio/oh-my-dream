//! Typed media and provider boundaries consumed by exact node capabilities.

#![deny(missing_docs)]

mod fake;
mod interface;
mod provider;
mod value;

pub use fake::*;
pub use interface::*;
pub use provider::*;
pub use value::*;
