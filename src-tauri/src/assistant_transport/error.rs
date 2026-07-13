use std::io;
use std::str::Utf8Error;

use thiserror::Error;

/// Concrete failures produced while validating or transferring assistant frames.
#[derive(Debug, Error)]
pub enum AssistantTransportError {
    #[error("assistant transport expected sequence {expected}, received {actual}")]
    UnexpectedSequence { expected: u64, actual: u64 },
    #[error("assistant transport sequence space is exhausted")]
    SequenceExhausted,
    #[error("assistant transport expected sequence {expected_sequence}, received EOF")]
    UnexpectedEof { expected_sequence: u64 },
    #[error("assistant transport received frame {sequence} after a terminal frame")]
    TrailingFrame { sequence: u64 },
    #[error("assistant transport frame ended after {bytes_read} bytes without a newline")]
    PartialFrameAtEof { bytes_read: usize },
    #[error("assistant transport frame is too large: observed {observed_size} bytes, max {max}")]
    FrameTooLarge { observed_size: usize, max: usize },
    #[error("assistant transport frame is not valid UTF-8")]
    InvalidUtf8 {
        #[source]
        source: Utf8Error,
    },
    #[error("assistant transport frame is malformed JSON")]
    MalformedJson {
        #[source]
        source: serde_json::Error,
    },
    #[error("assistant transport JSON number exceeds safe magnitude {maximum}")]
    JsonNumberOutOfRange { maximum: u64 },
    #[error("assistant transport JSON depth exceeds maximum {maximum}")]
    JsonDepthExceeded { maximum: usize },
    #[error("assistant transport frame top level must be an object")]
    TopLevelMustBeObject,
    #[error("assistant transport frame contains unknown top-level key `{key}`")]
    UnknownTopLevelKey { key: String },
    #[error("assistant transport frame is missing top-level key `{key}`")]
    MissingTopLevelKey { key: &'static str },
    #[error("assistant transport field `{field}` must be {expected}")]
    InvalidFieldType { field: &'static str, expected: &'static str },
    #[error("assistant transport protocol version {actual} is unsupported")]
    UnsupportedProtocolVersion { actual: u64 },
    #[error("assistant transport frame kind `{kind}` is unsupported")]
    UnknownKind { kind: String },
    #[error("assistant transport payload must be an object")]
    PayloadMustBeObject,
    #[error("assistant transport failed to read a frame")]
    Read {
        #[source]
        source: io::Error,
    },
    #[error("assistant transport failed to encode a frame")]
    Encode {
        #[source]
        source: serde_json::Error,
    },
    #[error("assistant transport failed to write a frame")]
    Write {
        #[source]
        source: io::Error,
    },
    #[error("assistant transport failed to flush a frame")]
    Flush {
        #[source]
        source: io::Error,
    },
    #[error("assistant transport endpoint is closed after a previous I/O or protocol failure")]
    Closed,
}
