//! Immutable reviewed Workflow change authority.

mod aggregate;
mod validation;
mod value;

pub use aggregate::*;
pub use value::*;

#[cfg(test)]
mod tests;
