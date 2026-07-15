//! Runtime values carried between nodes during execution.

use crate::port::PortType;
use std::collections::BTreeMap;

/// A concrete value produced or consumed on a port at runtime.
///
/// Media variants hold an opaque reference (an asset id or URL resolved by
/// other crates) rather than raw bytes: the engine is pure logic and never
/// touches the filesystem or network.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// UTF-8 text.
    String(String),
    /// Reference to an image (asset id / URL), resolved outside the engine.
    Image(String),
    /// Reference to a video (asset id / URL), resolved outside the engine.
    Video(String),
    /// Reference to an audio clip (asset id / URL), resolved outside the engine.
    Audio(String),
    /// A cloud model identifier.
    Model(String),
    /// A signed integer.
    Int(i64),
    /// A floating-point number.
    Float(f64),
}

impl Value {
    /// The [`PortType`] this value satisfies.
    #[must_use]
    pub fn port_type(&self) -> PortType {
        match self {
            Value::String(_) => PortType::String,
            Value::Image(_) => PortType::Image,
            Value::Video(_) => PortType::Video,
            Value::Audio(_) => PortType::Audio,
            Value::Model(_) => PortType::Model,
            Value::Int(_) => PortType::Int,
            Value::Float(_) => PortType::Float,
        }
    }
}

/// One runtime input with explicit cardinality.
#[derive(Debug, Clone, PartialEq)]
pub enum InputValue {
    /// Exactly one value.
    Single(Value),
    /// An ordered collection whose order is semantically significant.
    OrderedMany(Vec<Value>),
}

/// Named runtime inputs, ordered for deterministic cache hashing.
pub type NodeInputs = BTreeMap<String, InputValue>;

/// Named scalar outputs, ordered for deterministic cache hashing.
pub type ValueMap = BTreeMap<String, Value>;
