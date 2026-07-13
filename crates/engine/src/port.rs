//! Connection type system for the node graph.
//!
//! Every node port carries one of these data types. Two ports may be wired
//! together only when their [`PortType`] values are equal; this is checked at
//! graph-build time, never deferred to execution.

use serde::{Deserialize, Serialize};

/// Cardinality of a named workflow port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortCardinality {
    /// Exactly one value is accepted or produced.
    One,
    /// An ordered collection with explicit bounds is accepted.
    Many {
        /// Inclusive minimum number of values.
        minimum: usize,
        /// Inclusive maximum number of values, when bounded.
        maximum: Option<usize>,
    },
}

/// The data type flowing through a single port.
///
/// This is intentionally small: the first product milestone only needs to move
/// text, generated media, model identifiers, and a couple of numeric widget
/// values between nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    /// UTF-8 text (e.g. a prompt).
    String,
    /// A generated or loaded still image.
    Image,
    /// A generated video.
    Video,
    /// A generated audio clip.
    Audio,
    /// An opaque model identifier selecting a cloud model.
    Model,
    /// A signed integer widget value.
    Int,
    /// A floating-point widget value.
    Float,
}

impl PortType {
    /// Every supported port type in stable contract order.
    pub const ALL: [Self; 7] =
        [Self::String, Self::Image, Self::Video, Self::Audio, Self::Model, Self::Int, Self::Float];

    /// Returns whether a value of `self` may feed a port declared as `other`.
    ///
    /// Wiring is exact-match only for now; there is no implicit coercion. Kept
    /// as a method so future compatible-type rules land in one place.
    #[must_use]
    pub fn is_compatible_with(self, other: PortType) -> bool {
        self == other
    }
}
