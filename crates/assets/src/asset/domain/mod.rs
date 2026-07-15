//! Authoritative Asset domain values and aggregate semantics.

#![deny(missing_docs)]

mod content;
mod error;
mod identity;
mod integration;
mod media;
mod name;
mod origin;

pub use content::*;
pub use error::*;
pub use identity::*;
pub use integration::*;
pub use media::*;
pub use name::*;
pub use origin::*;
