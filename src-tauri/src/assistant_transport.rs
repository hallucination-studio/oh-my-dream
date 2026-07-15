mod codec;
mod error;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub use error::AssistantTransportError;

/// Wire protocol version supported by the Rust assistant transport.
pub const PROTOCOL_VERSION: u64 = 1;

/// Maximum encoded frame size, including the trailing newline.
pub const MAX_FRAME_SIZE: usize = 1_048_576;

/// Maximum nested JSON container depth, with the complete frame root at depth zero.
pub const MAX_JSON_DEPTH: usize = 64;

/// Largest integer magnitude represented exactly by the shared JSON number domain.
pub const MAX_SAFE_JSON_NUMBER: u64 = 9_007_199_254_740_991;

const RAW_DRAIN_BUFFER_SIZE: usize = 8 * 1_024;

/// Supported assistant protocol frame kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantFrameKind {
    Invoke,
    ResponsesEvent,
    ToolRequest,
    ToolResponse,
    ReviewSubmit,
    ReviewResponse,
    ReviewCheck,
    ReviewCheckResponse,
    ApprovalRequest,
    ApprovalResponse,
    Cancel,
    Snapshot,
    Completed,
    Error,
}

/// A validated v1 assistant protocol frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AssistantFrame {
    sequence: u64,
    kind: AssistantFrameKind,
    payload: Value,
}

impl AssistantFrame {
    /// Creates a frame whose payload is a JSON object.
    pub fn new(
        sequence: u64,
        kind: AssistantFrameKind,
        payload: Value,
    ) -> Result<Self, AssistantTransportError> {
        if !payload.is_object() {
            return Err(AssistantTransportError::PayloadMustBeObject);
        }
        Ok(Self { sequence, kind, payload })
    }

    /// Returns the fixed wire protocol version.
    pub fn protocol_version(&self) -> u64 {
        PROTOCOL_VERSION
    }

    /// Returns this frame's sequence number.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns this frame's kind.
    pub fn kind(&self) -> AssistantFrameKind {
        self.kind
    }

    /// Returns the payload without remapping it.
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// Stateful bounded async reader for newline-delimited assistant frames.
pub struct AssistantFrameReader<R> {
    reader: R,
    next_sequence: Option<u64>,
    closed: bool,
}

impl<R: AsyncBufRead + Unpin> AssistantFrameReader<R> {
    /// Creates a reader expecting sequence zero.
    pub fn new(reader: R) -> Self {
        Self { reader, next_sequence: Some(0), closed: false }
    }

    /// Reads and validates one complete frame.
    pub async fn read_frame(&mut self) -> Result<AssistantFrame, AssistantTransportError> {
        if self.closed {
            return Err(AssistantTransportError::Closed);
        }
        let result = match self.next_sequence {
            Some(expected) => match self.read_frame_or_eof(expected).await {
                Ok(Some(frame)) => Ok(frame),
                Ok(None) => {
                    Err(AssistantTransportError::UnexpectedEof { expected_sequence: expected })
                }
                Err(error) => Err(error),
            },
            None => Err(AssistantTransportError::SequenceExhausted),
        };
        match result {
            Ok(frame) => Ok(frame),
            Err(error) => {
                self.closed = true;
                Err(error)
            }
        }
    }

    /// Validates one frame before invoking the supplied operation callback.
    pub async fn read_and_dispatch<T>(
        &mut self,
        operation: impl FnOnce(AssistantFrame) -> T,
    ) -> Result<T, AssistantTransportError> {
        self.read_frame().await.map(operation)
    }

    pub(crate) async fn drain_to_eof(&mut self) -> Result<(), AssistantTransportError> {
        if self.closed {
            return Err(AssistantTransportError::Closed);
        }
        loop {
            let expected = match self.next_sequence {
                Some(expected) => expected,
                None => {
                    return self
                        .raw_drain_after_error(AssistantTransportError::SequenceExhausted)
                        .await;
                }
            };
            match self.read_frame_or_eof(expected).await {
                Ok(Some(_frame)) => {}
                Ok(None) => {
                    self.closed = true;
                    return Ok(());
                }
                Err(error) => {
                    return self.raw_drain_after_error(error).await;
                }
            }
        }
    }

    pub(crate) async fn expect_eof(&mut self) -> Result<(), AssistantTransportError> {
        if self.closed {
            return Err(AssistantTransportError::Closed);
        }
        let expected = self.next_sequence.ok_or(AssistantTransportError::SequenceExhausted)?;
        match self.read_frame_or_eof(expected).await {
            Ok(None) => {
                self.closed = true;
                Ok(())
            }
            Ok(Some(frame)) => {
                let error = AssistantTransportError::TrailingFrame { sequence: frame.sequence() };
                self.raw_drain_after_error(error).await
            }
            Err(error) => self.raw_drain_after_error(error).await,
        }
    }

    async fn raw_drain_after_error(
        &mut self,
        original_error: AssistantTransportError,
    ) -> Result<(), AssistantTransportError> {
        self.closed = true;
        let mut buffer = [0_u8; RAW_DRAIN_BUFFER_SIZE];
        loop {
            match self.reader.read(&mut buffer).await {
                Ok(0) => return Err(original_error),
                Ok(_bytes_read) => {}
                Err(source) => {
                    tracing::error!(
                        error = %source,
                        "failed to drain assistant transport after protocol error"
                    );
                    return Err(original_error);
                }
            }
        }
    }

    async fn read_frame_or_eof(
        &mut self,
        expected: u64,
    ) -> Result<Option<AssistantFrame>, AssistantTransportError> {
        let mut encoded = Vec::new();
        let mut bounded = (&mut self.reader).take((MAX_FRAME_SIZE + 1) as u64);
        bounded
            .read_until(b'\n', &mut encoded)
            .await
            .map_err(|source| AssistantTransportError::Read { source })?;
        if encoded.is_empty() {
            return Ok(None);
        }
        if encoded.len() > MAX_FRAME_SIZE {
            return Err(AssistantTransportError::FrameTooLarge {
                observed_size: encoded.len(),
                max: MAX_FRAME_SIZE,
            });
        }
        if encoded.last() != Some(&b'\n') {
            return Err(AssistantTransportError::PartialFrameAtEof { bytes_read: encoded.len() });
        }

        let json = std::str::from_utf8(&encoded[..encoded.len() - 1])
            .map_err(|source| AssistantTransportError::InvalidUtf8 { source })?;
        let frame = codec::decode_frame(json)?;
        if frame.sequence != expected {
            return Err(AssistantTransportError::UnexpectedSequence {
                expected,
                actual: frame.sequence,
            });
        }
        self.next_sequence = expected.checked_add(1);
        Ok(Some(frame))
    }
}

/// Stateful async writer for validated newline-delimited assistant frames.
pub struct AssistantFrameWriter<W> {
    writer: W,
    next_sequence: Option<u64>,
    closed: bool,
}

impl<W: AsyncWrite + Unpin> AssistantFrameWriter<W> {
    /// Creates a writer expecting sequence zero.
    pub fn new(writer: W) -> Self {
        Self { writer, next_sequence: Some(0), closed: false }
    }

    /// Validates, writes, and flushes one complete frame.
    pub async fn write_frame(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<(), AssistantTransportError> {
        if self.closed {
            return Err(AssistantTransportError::Closed);
        }
        let expected = self.next_sequence.ok_or(AssistantTransportError::SequenceExhausted)?;
        let encoded = codec::encode_frame(frame, expected)?;
        if let Err(source) = self.writer.write_all(&encoded).await {
            self.closed = true;
            return Err(AssistantTransportError::Write { source });
        }
        if let Err(source) = self.writer.flush().await {
            self.closed = true;
            return Err(AssistantTransportError::Flush { source });
        }
        self.next_sequence = expected.checked_add(1);
        Ok(())
    }

    /// Returns the wrapped writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}
