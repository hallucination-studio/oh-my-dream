//! Validated non-secret Desktop backend configuration and safe error boundary.

mod error;
mod interface;
mod json;
mod value;

pub use error::*;
pub use interface::*;
pub use value::*;

#[cfg(test)]
mod tests;
