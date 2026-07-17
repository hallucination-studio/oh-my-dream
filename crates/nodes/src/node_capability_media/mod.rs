//! Typed media and provider boundaries consumed by exact node capabilities.

#![deny(missing_docs)]

mod fake;
mod interface;
mod provider;
mod provider_fake;
mod value;

pub use fake::*;
pub use interface::*;
pub use provider::*;
pub use provider_fake::*;
pub use value::*;

fn byte_length_within_kind_limit(byte_length: u64, kind: NodeCapabilityMediaKind) -> bool {
    let maximum = match kind {
        NodeCapabilityMediaKind::Image => 32 * 1024 * 1024,
        NodeCapabilityMediaKind::Video => 512 * 1024 * 1024,
        NodeCapabilityMediaKind::Audio => 64 * 1024 * 1024,
    };
    byte_length > 0 && byte_length <= maximum
}
