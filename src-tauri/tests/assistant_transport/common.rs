use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use oh_my_dream_tauri::assistant_transport::{
    AssistantFrame, AssistantFrameReader, AssistantTransportError, PROTOCOL_VERSION,
};
use serde_json::{Value, json};
use tokio::io::{AsyncWrite, BufReader};

pub async fn read_one(input: Vec<u8>) -> Result<AssistantFrame, AssistantTransportError> {
    let mut reader = AssistantFrameReader::new(BufReader::new(std::io::Cursor::new(input)));
    reader.read_frame().await
}

pub fn wire_value(protocol_version: u64, sequence: u64, kind: &str, payload: Value) -> Value {
    json!({
        "protocol_version": protocol_version,
        "sequence": sequence,
        "kind": kind,
        "payload": payload
    })
}

pub fn json_line(value: Value) -> Vec<u8> {
    let mut encoded = serde_json::to_vec(&value).expect("test JSON should encode");
    encoded.push(b'\n');
    encoded
}

pub fn encoded_wire_frame(sequence: u64, kind: &str, payload: Value) -> Vec<u8> {
    json_line(wire_value(PROTOCOL_VERSION, sequence, kind, payload))
}

#[derive(Default)]
pub struct TestWriter {
    pub bytes: Vec<u8>,
    pub flushes: usize,
    write_limit: Option<usize>,
    fail_flush: bool,
}

impl TestWriter {
    pub fn partial_then_fail(write_limit: usize) -> Self {
        Self { write_limit: Some(write_limit), ..Self::default() }
    }

    pub fn fail_flush() -> Self {
        Self { fail_flush: true, ..Self::default() }
    }
}

impl AsyncWrite for TestWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let writer = self.get_mut();
        if let Some(limit) = writer.write_limit {
            if writer.bytes.len() >= limit {
                return Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed")));
            }
            let accepted = (limit - writer.bytes.len()).min(buffer.len());
            writer.bytes.extend_from_slice(&buffer[..accepted]);
            return Poll::Ready(Ok(accepted));
        }
        writer.bytes.extend_from_slice(buffer);
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let writer = self.get_mut();
        writer.flushes += 1;
        if writer.fail_flush {
            Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush failed")))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
