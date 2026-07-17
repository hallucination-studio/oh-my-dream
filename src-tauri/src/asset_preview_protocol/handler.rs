use std::{collections::BTreeMap, sync::Arc, time::Instant};

use assets::asset::{
    application::{
        AssetApplicationError, AssetGetQuery, AssetGetUseCase, AssetPreviewLease,
        AssetResolveContentQuery, AssetResolveContentUseCase,
    },
    domain::AssetMediaKind,
    interfaces::AssetClockInterface,
};
use tokio::io::AsyncReadExt;

use super::{DecodedPreviewToken, PreviewTokenCodec, token::TokenError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Supported desktop preview protocol methods.
pub enum DesktopAssetPreviewMethod {
    /// Retrieve headers and content.
    Get,
    /// Retrieve the same headers without content.
    Head,
    /// Any method outside the frozen protocol.
    Unsupported,
}

/// One deadline-bounded desktop preview protocol request.
pub struct DesktopAssetPreviewProtocolRequest {
    /// Frozen method category.
    pub method: DesktopAssetPreviewMethod,
    /// Opaque signed protocol URI.
    pub uri: String,
    /// Optional single byte range.
    pub range: Option<String>,
    /// Process-monotonic storage deadline.
    pub deadline: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Transport-neutral desktop preview protocol response.
pub struct DesktopAssetPreviewProtocolResponse {
    /// HTTP-compatible status code.
    pub status: u16,
    /// Frozen response headers.
    pub headers: BTreeMap<String, String>,
    /// Empty HEAD body or selected GET bytes.
    pub body: Vec<u8>,
}

/// Process-keyed signed preview issuance and protocol handling adapter.
pub struct DesktopAssetPreviewProtocolAdapterImpl {
    codec: PreviewTokenCodec,
    get_asset: Arc<AssetGetUseCase>,
    resolve_content: Arc<AssetResolveContentUseCase>,
    clock: Arc<dyn AssetClockInterface>,
}

impl DesktopAssetPreviewProtocolAdapterImpl {
    /// Creates a protocol adapter with a fresh process-local CSPRNG key.
    pub fn try_new(
        get_asset: Arc<AssetGetUseCase>,
        resolve_content: Arc<AssetResolveContentUseCase>,
        clock: Arc<dyn AssetClockInterface>,
    ) -> Result<Self, AssetApplicationError> {
        Ok(Self {
            codec: PreviewTokenCodec::generate()
                .map_err(|_| AssetApplicationError::PreviewLeaseInvalid)?,
            get_asset,
            resolve_content,
            clock,
        })
    }

    #[must_use]
    /// Issues the canonical signed URI for one exact preview lease.
    pub fn issue_preview_uri(&self, lease: &AssetPreviewLease) -> String {
        self.codec.issue(lease)
    }

    /// Handles one signed protocol request without exposing filesystem paths.
    pub async fn handle(
        &self,
        request: DesktopAssetPreviewProtocolRequest,
    ) -> DesktopAssetPreviewProtocolResponse {
        if request.method == DesktopAssetPreviewMethod::Unsupported {
            return response(405);
        }
        let now = match self.clock.current_asset_time() {
            Ok(value) => value.as_utc_milliseconds(),
            Err(_) => return response(500),
        };
        let token = match self.codec.decode(&request.uri, now) {
            Ok(token) => token,
            Err(TokenError::Expired) => return response(410),
            Err(TokenError::Invalid) => return response(404),
        };
        match self.resolve(token, request).await {
            Ok(response) => response,
            Err(status) => response(status),
        }
    }

    async fn resolve(
        &self,
        token: DecodedPreviewToken,
        request: DesktopAssetPreviewProtocolRequest,
    ) -> Result<DesktopAssetPreviewProtocolResponse, u16> {
        let asset = self
            .get_asset
            .get_asset(AssetGetQuery::new(token.project_id, token.asset_id))
            .await
            .map_err(asset_status)?;
        let media_kind = asset.media_kind();
        let resolved = self
            .resolve_content
            .resolve_asset_content(AssetResolveContentQuery::new(
                token.project_id,
                token.asset_id,
                media_kind,
                request.deadline,
            ))
            .await
            .map_err(asset_status)?;
        let descriptor = resolved.descriptor().clone();
        if descriptor.content_id() != token.content_id {
            return Err(404);
        }
        let mut stream = resolved.into_content_lease().try_take_stream().map_err(asset_status)?;
        let mut bytes =
            Vec::with_capacity(usize::try_from(descriptor.byte_length()).map_err(|_| 500_u16)?);
        stream.read_to_end(&mut bytes).await.map_err(|_| 500_u16)?;
        if bytes.len() as u64 != descriptor.byte_length() {
            return Err(500);
        }
        build_success(request.method, request.range.as_deref(), media_kind, descriptor, bytes)
    }
}

pub(super) fn build_success(
    method: DesktopAssetPreviewMethod,
    range: Option<&str>,
    media_kind: AssetMediaKind,
    descriptor: assets::asset::domain::AssetContentDescriptor,
    bytes: Vec<u8>,
) -> Result<DesktopAssetPreviewProtocolResponse, u16> {
    if media_kind == AssetMediaKind::Image && range.is_some() {
        return Err(416);
    }
    let selected = match range {
        Some(value) => parse_range(value, bytes.len()).ok_or(416_u16)?,
        None => (0, bytes.len().saturating_sub(1)),
    };
    let partial = range.is_some();
    let body = if method == DesktopAssetPreviewMethod::Head {
        Vec::new()
    } else if bytes.is_empty() {
        return Err(500);
    } else {
        bytes[selected.0..=selected.1].to_vec()
    };
    let content_length = selected.1 - selected.0 + 1;
    let mut headers = BTreeMap::from([
        ("Content-Type".to_owned(), descriptor.mime_type().as_str().to_owned()),
        ("Content-Length".to_owned(), content_length.to_string()),
        ("ETag".to_owned(), digest_hex(descriptor.digest().as_bytes())),
        ("Cache-Control".to_owned(), "private, no-store".to_owned()),
        ("X-Content-Type-Options".to_owned(), "nosniff".to_owned()),
    ]);
    if media_kind != AssetMediaKind::Image {
        headers.insert("Accept-Ranges".to_owned(), "bytes".to_owned());
    }
    if partial {
        headers.insert(
            "Content-Range".to_owned(),
            format!("bytes {}-{}/{}", selected.0, selected.1, bytes.len()),
        );
    }
    Ok(DesktopAssetPreviewProtocolResponse {
        status: if partial { 206 } else { 200 },
        headers,
        body,
    })
}

pub(super) fn parse_range(value: &str, length: usize) -> Option<(usize, usize)> {
    if length == 0 || value.contains(',') {
        return None;
    }
    let value = value.strip_prefix("bytes=")?;
    let (start, end) = value.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<usize>().ok()?;
        return (suffix > 0).then(|| (length.saturating_sub(suffix.min(length)), length - 1));
    }
    let start = start.parse::<usize>().ok()?;
    let end = if end.is_empty() { length - 1 } else { end.parse::<usize>().ok()? };
    (start <= end && end < length).then_some((start, end))
}

fn asset_status(error: AssetApplicationError) -> u16 {
    match error {
        AssetApplicationError::NotFound
        | AssetApplicationError::NotVisible
        | AssetApplicationError::MediaKindMismatch { .. }
        | AssetApplicationError::ContentPending
        | AssetApplicationError::ContentMissing => 404,
        AssetApplicationError::PreviewLeaseExpired => 410,
        AssetApplicationError::PreviewRangeInvalid => 416,
        _ => 500,
    }
}

fn digest_hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn response(status: u16) -> DesktopAssetPreviewProtocolResponse {
    DesktopAssetPreviewProtocolResponse { status, headers: BTreeMap::new(), body: Vec::new() }
}
