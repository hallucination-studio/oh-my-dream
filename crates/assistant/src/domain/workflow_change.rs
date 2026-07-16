//! Immutable reviewed Workflow change authority.

mod aggregate;
mod value;

pub use aggregate::*;
pub use value::*;

#[cfg(test)]
mod tests;
