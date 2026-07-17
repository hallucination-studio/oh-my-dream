//! Provider-independent Generation Profile identity, catalog, and availability queries.

#![deny(missing_docs)]

mod application;
mod availability;
mod catalog;
mod identity;

pub use application::*;
pub use availability::*;
pub use catalog::*;
pub use identity::*;
