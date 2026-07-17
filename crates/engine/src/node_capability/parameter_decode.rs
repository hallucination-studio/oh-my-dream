//! Canonical parameter-set decoding for persistence boundaries.

use std::collections::BTreeMap;

use super::{
    NodeCapabilityChoiceKey, NodeCapabilityGenerationProfileRefParameterValue,
    NodeCapabilityManagedAssetIdParameterValue, NodeCapabilityParameterKey,
    NodeCapabilityParameterSet, NodeCapabilityParameterValue, WorkflowManagedAssetIdBoundaryValue,
};

const MAX_CANONICAL_BYTES: usize = 1024 * 1024;
const MAX_VARIABLE_BYTES: usize = 64 * 1024;

/// Bounded category for an invalid canonical parameter-set encoding.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum NodeCapabilityParameterCanonicalDecodeErrorCategory {
    /// The complete input exceeds the frozen one-MiB bound.
    #[error("canonical parameter input is too large")]
    InputTooLarge,
    /// A length or fixed-width value extends beyond the input.
    #[error("canonical parameter input is truncated")]
    Truncated,
    /// A variable-width value exceeds the frozen allocation bound.
    #[error("canonical parameter value is too large")]
    ValueTooLarge,
    /// A string contains invalid UTF-8.
    #[error("canonical parameter string is invalid UTF-8")]
    InvalidUtf8,
    /// A decoded parameter or choice key violates its authoritative shape.
    #[error("canonical parameter key is invalid")]
    InvalidKey,
    /// A decoded cross-context boundary value violates its canonical shape.
    #[error("canonical parameter boundary value is invalid")]
    InvalidBoundaryValue,
    /// The entry count exceeds the frozen set bound.
    #[error("canonical parameter set is too large")]
    ParameterSetTooLarge,
    /// Entries are not strictly ordered by parameter key.
    #[error("canonical parameter keys are not strictly ordered")]
    NonCanonicalKeyOrder,
    /// A value tag is outside the closed parameter union.
    #[error("canonical parameter value tag is unknown")]
    UnknownValueTag,
    /// Bytes remain after the declared entries.
    #[error("canonical parameter input has trailing bytes")]
    TrailingBytes,
    /// Decoding and re-encoding do not produce identical bytes.
    #[error("canonical parameter input is not canonical")]
    NonCanonicalEncoding,
}

/// Location-aware failure to restore a canonical parameter set.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("{category} at byte offset {offset}")]
pub struct NodeCapabilityParameterCanonicalDecodeError {
    category: NodeCapabilityParameterCanonicalDecodeErrorCategory,
    offset: usize,
}

impl NodeCapabilityParameterCanonicalDecodeError {
    /// Returns the stable bounded failure category.
    #[must_use]
    pub const fn category(&self) -> NodeCapabilityParameterCanonicalDecodeErrorCategory {
        self.category
    }

    /// Returns the byte offset at which decoding failed.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }
}

impl NodeCapabilityParameterSet {
    /// Restores a parameter set from its exact canonical byte encoding.
    pub fn try_from_canonical_bytes(
        bytes: &[u8],
    ) -> Result<Self, NodeCapabilityParameterCanonicalDecodeError> {
        if bytes.len() > MAX_CANONICAL_BYTES {
            return Err(decode_error(
                NodeCapabilityParameterCanonicalDecodeErrorCategory::InputTooLarge,
                0,
            ));
        }
        let mut decoder = ParameterDecoder::new(bytes);
        let count = decoder.read_u32()? as usize;
        if count > 64 {
            return Err(decoder
                .error(NodeCapabilityParameterCanonicalDecodeErrorCategory::ParameterSetTooLarge));
        }
        let mut values = BTreeMap::new();
        let mut previous_key: Option<NodeCapabilityParameterKey> = None;
        for _ in 0..count {
            let key_offset = decoder.offset;
            let key = decoder.read_parameter_key()?;
            if previous_key.as_ref().is_some_and(|previous| previous >= &key) {
                return Err(decode_error(
                    NodeCapabilityParameterCanonicalDecodeErrorCategory::NonCanonicalKeyOrder,
                    key_offset,
                ));
            }
            let value = decoder.read_parameter_value()?;
            previous_key = Some(key.clone());
            values.insert(key, value);
        }
        if decoder.offset != bytes.len() {
            return Err(
                decoder.error(NodeCapabilityParameterCanonicalDecodeErrorCategory::TrailingBytes)
            );
        }
        let parameter_set = Self::try_from_map(values).map_err(|_| {
            decoder.error(NodeCapabilityParameterCanonicalDecodeErrorCategory::ParameterSetTooLarge)
        })?;
        if parameter_set.canonical_bytes() != bytes {
            return Err(decode_error(
                NodeCapabilityParameterCanonicalDecodeErrorCategory::NonCanonicalEncoding,
                0,
            ));
        }
        Ok(parameter_set)
    }
}

struct ParameterDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ParameterDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_parameter_key(
        &mut self,
    ) -> Result<NodeCapabilityParameterKey, NodeCapabilityParameterCanonicalDecodeError> {
        let offset = self.offset;
        let value = self.read_string()?;
        NodeCapabilityParameterKey::new(value).map_err(|_| {
            decode_error(NodeCapabilityParameterCanonicalDecodeErrorCategory::InvalidKey, offset)
        })
    }

    fn read_parameter_value(
        &mut self,
    ) -> Result<NodeCapabilityParameterValue, NodeCapabilityParameterCanonicalDecodeError> {
        let tag_offset = self.offset;
        let tag = self.read_exact(1)?[0];
        match tag {
            0 => Ok(NodeCapabilityParameterValue::UnsignedInteger(self.read_u64()?)),
            1 => Ok(NodeCapabilityParameterValue::Text(self.read_string()?)),
            2 => self.read_choice(),
            3 => self.read_generation_profile(),
            4 => self.read_managed_asset(),
            _ => Err(decode_error(
                NodeCapabilityParameterCanonicalDecodeErrorCategory::UnknownValueTag,
                tag_offset,
            )),
        }
    }

    fn read_choice(
        &mut self,
    ) -> Result<NodeCapabilityParameterValue, NodeCapabilityParameterCanonicalDecodeError> {
        let offset = self.offset;
        let value = self.read_string()?;
        NodeCapabilityChoiceKey::new(value).map(NodeCapabilityParameterValue::Choice).map_err(
            |_| {
                decode_error(
                    NodeCapabilityParameterCanonicalDecodeErrorCategory::InvalidKey,
                    offset,
                )
            },
        )
    }

    fn read_generation_profile(
        &mut self,
    ) -> Result<NodeCapabilityParameterValue, NodeCapabilityParameterCanonicalDecodeError> {
        let offset = self.offset;
        let profile_id = self.read_string()?;
        let version = self.read_u32()?;
        NodeCapabilityGenerationProfileRefParameterValue::new(profile_id, version)
            .map(NodeCapabilityParameterValue::GenerationProfile)
            .map_err(|_| {
                decode_error(
                    NodeCapabilityParameterCanonicalDecodeErrorCategory::InvalidBoundaryValue,
                    offset,
                )
            })
    }

    fn read_managed_asset(
        &mut self,
    ) -> Result<NodeCapabilityParameterValue, NodeCapabilityParameterCanonicalDecodeError> {
        let offset = self.offset;
        let bytes = self.read_exact(16)?;
        let mut asset_id = [0_u8; 16];
        asset_id.copy_from_slice(bytes);
        WorkflowManagedAssetIdBoundaryValue::from_bytes(asset_id)
            .map(NodeCapabilityManagedAssetIdParameterValue::new)
            .map(NodeCapabilityParameterValue::ManagedAsset)
            .map_err(|_| {
                decode_error(
                    NodeCapabilityParameterCanonicalDecodeErrorCategory::InvalidBoundaryValue,
                    offset,
                )
            })
    }

    fn read_string(&mut self) -> Result<String, NodeCapabilityParameterCanonicalDecodeError> {
        let offset = self.offset;
        let length = self.read_u32()? as usize;
        if length > MAX_VARIABLE_BYTES {
            return Err(decode_error(
                NodeCapabilityParameterCanonicalDecodeErrorCategory::ValueTooLarge,
                offset,
            ));
        }
        let value = self.read_exact(length)?;
        std::str::from_utf8(value).map(str::to_owned).map_err(|_| {
            decode_error(NodeCapabilityParameterCanonicalDecodeErrorCategory::InvalidUtf8, offset)
        })
    }

    fn read_u32(&mut self) -> Result<u32, NodeCapabilityParameterCanonicalDecodeError> {
        let bytes = self.read_exact(4)?;
        let mut value = [0_u8; 4];
        value.copy_from_slice(bytes);
        Ok(u32::from_be_bytes(value))
    }

    fn read_u64(&mut self) -> Result<u64, NodeCapabilityParameterCanonicalDecodeError> {
        let bytes = self.read_exact(8)?;
        let mut value = [0_u8; 8];
        value.copy_from_slice(bytes);
        Ok(u64::from_be_bytes(value))
    }

    fn read_exact(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], NodeCapabilityParameterCanonicalDecodeError> {
        let end = self.offset.checked_add(length).ok_or_else(|| {
            self.error(NodeCapabilityParameterCanonicalDecodeErrorCategory::Truncated)
        })?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| {
            self.error(NodeCapabilityParameterCanonicalDecodeErrorCategory::Truncated)
        })?;
        self.offset = end;
        Ok(value)
    }

    const fn error(
        &self,
        category: NodeCapabilityParameterCanonicalDecodeErrorCategory,
    ) -> NodeCapabilityParameterCanonicalDecodeError {
        decode_error(category, self.offset)
    }
}

const fn decode_error(
    category: NodeCapabilityParameterCanonicalDecodeErrorCategory,
    offset: usize,
) -> NodeCapabilityParameterCanonicalDecodeError {
    NodeCapabilityParameterCanonicalDecodeError { category, offset }
}
