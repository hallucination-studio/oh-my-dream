use assets::asset::{
    application::AssetPreviewLease,
    domain::{AssetContentDigest, AssetId, AssetManagedContentId},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};

const VERSION: u8 = 1;
const KEY_BYTES: usize = 32;
const PAYLOAD_BYTES: usize = 98;
const TOKEN_BYTES: usize = PAYLOAD_BYTES + 32;

pub(super) struct PreviewTokenCodec {
    key: [u8; KEY_BYTES],
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DecodedPreviewToken {
    pub project_id: ProjectId,
    pub asset_id: AssetId,
    pub content_id: AssetManagedContentId,
    pub issued_at_epoch_ms: i64,
    pub expires_at_epoch_ms: i64,
}

impl PreviewTokenCodec {
    pub fn generate() -> Result<Self, ()> {
        let mut key = [0_u8; KEY_BYTES];
        getrandom::getrandom(&mut key).map_err(|_| ())?;
        Ok(Self { key })
    }

    #[cfg(test)]
    pub const fn from_test_key(key: [u8; KEY_BYTES]) -> Self {
        Self { key }
    }

    pub fn issue(&self, lease: &AssetPreviewLease) -> String {
        let mut payload = Vec::with_capacity(PAYLOAD_BYTES);
        payload.push(VERSION);
        payload.extend_from_slice(lease.lease_id().as_uuid().as_bytes());
        payload.extend_from_slice(lease.project_id().as_uuid().as_bytes());
        payload.extend_from_slice(lease.asset_id().as_uuid().as_bytes());
        payload.extend_from_slice(&lease.content_id().canonical_bytes());
        payload.extend_from_slice(&lease.issued_at_utc_milliseconds().to_be_bytes());
        payload.extend_from_slice(&lease.expires_at_utc_milliseconds().to_be_bytes());
        let signature = hmac_sha256(&self.key, &payload);
        payload.extend_from_slice(&signature);
        format!("desktop-asset://v1/{}", URL_SAFE_NO_PAD.encode(payload))
    }

    pub fn decode(&self, uri: &str, now_epoch_ms: i64) -> Result<DecodedPreviewToken, TokenError> {
        let encoded = uri.strip_prefix("desktop-asset://v1/").ok_or(TokenError::Invalid)?;
        if encoded.is_empty() || encoded.contains('=') {
            return Err(TokenError::Invalid);
        }
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| TokenError::Invalid)?;
        if bytes.len() != TOKEN_BYTES || URL_SAFE_NO_PAD.encode(&bytes) != encoded {
            return Err(TokenError::Invalid);
        }
        let (payload, signature) = bytes.split_at(PAYLOAD_BYTES);
        if !constant_time_equal(&hmac_sha256(&self.key, payload), signature) {
            return Err(TokenError::Invalid);
        }
        parse_verified_payload(payload, now_epoch_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TokenError {
    Invalid,
    Expired,
}

fn parse_verified_payload(
    payload: &[u8],
    now_epoch_ms: i64,
) -> Result<DecodedPreviewToken, TokenError> {
    if payload[0] != VERSION || now_epoch_ms < 0 {
        return Err(TokenError::Invalid);
    }
    let project_id = ProjectId::from_uuid(uuid_at(payload, 17)?).ok_or(TokenError::Invalid)?;
    let asset_id = AssetId::from_uuid(uuid_at(payload, 33)?).map_err(|_| TokenError::Invalid)?;
    let content = array_at::<33>(payload, 49)?;
    if content[0] != VERSION {
        return Err(TokenError::Invalid);
    }
    let content_id = AssetManagedContentId::from_digest(AssetContentDigest::from_bytes(
        content[1..].try_into().map_err(|_| TokenError::Invalid)?,
    ));
    let issued_at_epoch_ms = i64::from_be_bytes(array_at(payload, 82)?);
    let expires_at_epoch_ms = i64::from_be_bytes(array_at(payload, 90)?);
    if issued_at_epoch_ms < 0
        || issued_at_epoch_ms > now_epoch_ms
        || issued_at_epoch_ms.checked_add(300_000) != Some(expires_at_epoch_ms)
    {
        return Err(TokenError::Invalid);
    }
    if now_epoch_ms >= expires_at_epoch_ms {
        return Err(TokenError::Expired);
    }
    Ok(DecodedPreviewToken {
        project_id,
        asset_id,
        content_id,
        issued_at_epoch_ms,
        expires_at_epoch_ms,
    })
}

fn uuid_at(payload: &[u8], start: usize) -> Result<uuid::Uuid, TokenError> {
    Ok(uuid::Uuid::from_bytes(array_at(payload, start)?))
}

fn array_at<const N: usize>(payload: &[u8], start: usize) -> Result<[u8; N], TokenError> {
    payload
        .get(start..start + N)
        .ok_or(TokenError::Invalid)?
        .try_into()
        .map_err(|_| TokenError::Invalid)
}

fn hmac_sha256(key: &[u8; KEY_BYTES], message: &[u8]) -> [u8; 32] {
    let mut inner_key = [0x36_u8; 64];
    let mut outer_key = [0x5c_u8; 64];
    for (index, byte) in key.iter().enumerate() {
        inner_key[index] ^= byte;
        outer_key[index] ^= byte;
    }
    let inner = Sha256::new().chain_update(inner_key).chain_update(message).finalize();
    Sha256::new().chain_update(outer_key).chain_update(inner).finalize().into()
}

fn constant_time_equal(expected: &[u8], observed: &[u8]) -> bool {
    if expected.len() != observed.len() {
        return false;
    }
    expected
        .iter()
        .zip(observed)
        .fold(0_u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}
