//! Project domain failures.

/// A rejected Project value or state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectDomainError {
    /// A normalized Project name was empty.
    #[error("Project name must not be empty")]
    ProjectNameEmpty,
    /// A Project name exceeded 120 Unicode scalar values.
    #[error("Project name must not exceed 120 Unicode scalar values")]
    ProjectNameTooLong,
    /// A Project name contained a control character.
    #[error("Project name must not contain control characters")]
    ProjectNameContainsControl,
    /// A rename normalized to the current Project name.
    #[error("Project name is unchanged")]
    ProjectNameUnchanged,
    /// A Project revision could not be incremented.
    #[error("Project revision overflow")]
    ProjectRevisionOverflow,
    /// A Project timestamp value or aggregate ordering was outside its valid range.
    #[error("Project timestamp is outside the valid range")]
    ProjectTimestampOutOfRange,
    /// A monotonic Project timestamp could not be produced.
    #[error("Project timestamp overflow")]
    ProjectTimestampOverflow,
}
