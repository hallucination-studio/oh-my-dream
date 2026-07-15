//! Frozen scalar Workflow graph values.

use super::WorkflowGraphConstructionError;

/// Hard-cut persisted Workflow schema version.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowSchemaVersion(u16);

impl WorkflowSchemaVersion {
    /// Current and only supported MVP schema.
    pub const CURRENT: Self = Self(1);

    /// Restores only the current hard-cut schema.
    pub const fn new(value: u16) -> Result<Self, WorkflowGraphConstructionError> {
        if value == Self::CURRENT.0 {
            Ok(Self(value))
        } else {
            Err(WorkflowGraphConstructionError::SchemaVersionUnsupported)
        }
    }

    /// Returns the stored schema number.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

macro_rules! workflow_timestamp {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i64);

        impl $name {
            /// Restores a non-negative UTC millisecond timestamp.
            pub const fn from_utc_milliseconds(
                value: i64,
            ) -> Result<Self, WorkflowGraphConstructionError> {
                if value < 0 {
                    Err(WorkflowGraphConstructionError::TimestampOutOfRange)
                } else {
                    Ok(Self(value))
                }
            }

            /// Returns UTC milliseconds.
            #[must_use]
            pub const fn as_utc_milliseconds(self) -> i64 {
                self.0
            }
        }
    };
}

workflow_timestamp!(WorkflowCreatedAt, "Immutable Workflow creation timestamp.");
workflow_timestamp!(WorkflowUpdatedAt, "Latest successful Workflow mutation timestamp.");

/// Persisted canvas position excluded from readiness and execution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkflowCanvasPosition {
    x: f64,
    y: f64,
}

impl WorkflowCanvasPosition {
    const LIMIT: f64 = 1_000_000.0;

    /// Creates a finite bounded position and normalizes negative zero.
    pub fn try_new(x: f64, y: f64) -> Result<Self, WorkflowGraphConstructionError> {
        if !Self::valid_coordinate(x) || !Self::valid_coordinate(y) {
            return Err(WorkflowGraphConstructionError::CanvasPositionOutOfBounds);
        }
        Ok(Self { x: Self::normalize_zero(x), y: Self::normalize_zero(y) })
    }

    /// Returns the normalized horizontal coordinate.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Returns the normalized vertical coordinate.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }

    fn valid_coordinate(value: f64) -> bool {
        value.is_finite() && (-Self::LIMIT..=Self::LIMIT).contains(&value)
    }

    fn normalize_zero(value: f64) -> f64 {
        if value == 0.0 { 0.0 } else { value }
    }
}
