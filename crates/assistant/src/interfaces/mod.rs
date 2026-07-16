//! Consumer-owned Assistant substitution boundaries.

mod bridge;
mod model;
mod repository;
mod value;

pub use bridge::*;
pub use model::*;
pub use repository::*;
pub use value::*;

#[cfg(test)]
mod fault_tests;
#[cfg(test)]
mod tests;
