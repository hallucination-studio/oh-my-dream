//! Project aggregate and value contracts.

mod aggregate;
mod error;
mod value;

pub use aggregate::ProjectAggregate;
pub use error::ProjectDomainError;
pub use value::{ProjectCreatedAt, ProjectId, ProjectName, ProjectRevision, ProjectUpdatedAt};
