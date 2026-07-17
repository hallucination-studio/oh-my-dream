//! Authoritative Asset domain values and aggregate semantics.

#![deny(missing_docs)]

mod aggregate;
mod content;
mod error;
mod identity;
mod integration;
mod media;
mod name;
mod origin;
mod state;

pub use aggregate::*;
pub use content::*;
pub use error::*;
pub use identity::*;
pub use integration::*;
pub use media::*;
pub use name::*;
pub use origin::*;
pub use state::*;
