use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use reqwest::{Method, StatusCode, Url, header};

use crate::credential_vault::{
    GenerationProviderCredentialId, GenerationProviderCredentialSecret,
    GenerationProviderCredentialVaultError, GenerationProviderCredentialVaultInterface,
};

const MAX_JSON_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

pub(super) struct FalGenerationProviderAccount {
    credential_id: GenerationProviderCredentialId,
    vault: Arc<dyn GenerationProviderCredentialVaultInterface>,
}

impl FalGenerationProviderAccount {
    pub(super) fn new(
        credential_id: GenerationProviderCredentialId,
        vault: Arc<dyn GenerationProviderCredentialVaultInterface>,
    ) -> Self {
        Self { credential_id, vault }
    }

    pub(super) async fn materialize(
        &self,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialVaultError> {
        self.vault.load_generation_provider_credential(&self.credential_id).await
    }

    pub(super) async fn credential_state(
        &self,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        self.materialize().await.map(drop)
    }
}

#[derive(Clone, Copy)]
pub(super) enum FalQueueMethod {
    Post,
    Get,
}

pub(super) struct FalQueueRequest {
    pub method: FalQueueMethod,
    pub url: String,
    pub json_body: Option<Vec<u8>>,
    pub deadline: Instant,
    pub submission_already_accepted: bool,
}

pub(super) struct FalHttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FalTransportError {
    DeadlineBeforeSubmission,
    DeadlineAfterSubmission,
    NetworkBeforeSubmission,
    NetworkAfterSubmission,
    ResponseTooLarge,
    DownloadRejected,
    InvalidCredential,
}

#[async_trait]
pub(super) trait FalHttpTransportInterface: Send + Sync {
    async fn send_queue_request(
        &self,
        request: FalQueueRequest,
        secret: &GenerationProviderCredentialSecret,
    ) -> Result<FalHttpResponse, FalTransportError>;

    async fn download_media(
        &self,
        url: &str,
        deadline: Instant,
    ) -> Result<FalHttpResponse, FalTransportError>;
}

pub(super) struct ReqwestFalHttpTransport {
    queue_client: reqwest::Client,
}

impl ReqwestFalHttpTransport {
    pub(super) fn try_new() -> Result<Self, reqwest::Error> {
        let queue_client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()?;
        Ok(Self { queue_client })
    }
}

#[async_trait]
impl FalHttpTransportInterface for ReqwestFalHttpTransport {
    async fn send_queue_request(
        &self,
        request: FalQueueRequest,
        secret: &GenerationProviderCredentialSecret,
    ) -> Result<FalHttpResponse, FalTransportError> {
        let timeout = remaining(request.deadline, FalTransportError::DeadlineBeforeSubmission)?;
        let method = match request.method {
            FalQueueMethod::Post => Method::POST,
            FalQueueMethod::Get => Method::GET,
        };
        let accepted =
            request.submission_already_accepted || matches!(request.method, FalQueueMethod::Post);
        let mut authorization_bytes = Vec::with_capacity(4 + secret.as_bytes().len());
        authorization_bytes.extend_from_slice(b"Key ");
        authorization_bytes.extend_from_slice(secret.as_bytes());
        let authorization_result = header::HeaderValue::from_bytes(&authorization_bytes);
        authorization_bytes.fill(0);
        let mut authorization =
            authorization_result.map_err(|_| FalTransportError::InvalidCredential)?;
        authorization.set_sensitive(true);
        let mut builder = self
            .queue_client
            .request(method, request.url)
            .header(header::AUTHORIZATION, authorization)
            .timeout(timeout);
        if let Some(body) = request.json_body {
            builder = builder.header(header::CONTENT_TYPE, "application/json").body(body);
        }
        let response = builder.send().await.map_err(|error| transport_error(error, accepted))?;
        read_response(response, MAX_JSON_RESPONSE_BYTES, accepted).await
    }

    async fn download_media(
        &self,
        url: &str,
        deadline: Instant,
    ) -> Result<FalHttpResponse, FalTransportError> {
        let mut current = validate_media_url(url)?;
        for redirect_count in 0..=3 {
            let host = current.host_str().ok_or(FalTransportError::DownloadRejected)?;
            let address = resolve_public_address(host).await?;
            let client = reqwest::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .resolve(host, address)
                .build()
                .map_err(|_| FalTransportError::DownloadRejected)?;
            let response = client
                .get(current.clone())
                .timeout(remaining(deadline, FalTransportError::DownloadRejected)?)
                .send()
                .await
                .map_err(|_| FalTransportError::DownloadRejected)?;
            if response.status().is_redirection() {
                if redirect_count == 3 {
                    return Err(FalTransportError::DownloadRejected);
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(FalTransportError::DownloadRejected)?;
                current = validate_media_url(
                    current
                        .join(location)
                        .map_err(|_| FalTransportError::DownloadRejected)?
                        .as_str(),
                )?;
                continue;
            }
            return read_response(response, MAX_IMAGE_BYTES, false)
                .await
                .map_err(|_| FalTransportError::DownloadRejected);
        }
        Err(FalTransportError::DownloadRejected)
    }
}

async fn read_response(
    mut response: reqwest::Response,
    maximum: usize,
    accepted: bool,
) -> Result<FalHttpResponse, FalTransportError> {
    if response.status().is_redirection() {
        return Err(if accepted {
            FalTransportError::NetworkAfterSubmission
        } else {
            FalTransportError::DownloadRejected
        });
    }
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        if accepted {
            FalTransportError::NetworkAfterSubmission
        } else {
            FalTransportError::NetworkBeforeSubmission
        }
    })? {
        if body.len().saturating_add(chunk.len()) > maximum {
            return Err(FalTransportError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(FalHttpResponse { status, content_type, body })
}

fn transport_error(error: reqwest::Error, accepted: bool) -> FalTransportError {
    if error.is_timeout() {
        if accepted {
            FalTransportError::DeadlineAfterSubmission
        } else {
            FalTransportError::DeadlineBeforeSubmission
        }
    } else if accepted {
        FalTransportError::NetworkAfterSubmission
    } else {
        FalTransportError::NetworkBeforeSubmission
    }
}

fn remaining(deadline: Instant, error: FalTransportError) -> Result<Duration, FalTransportError> {
    deadline.checked_duration_since(Instant::now()).filter(|value| !value.is_zero()).ok_or(error)
}

fn validate_media_url(value: &str) -> Result<Url, FalTransportError> {
    let url = Url::parse(value).map_err(|_| FalTransportError::DownloadRejected)?;
    let host = url.host_str().ok_or(FalTransportError::DownloadRejected)?;
    if url.scheme() != "https"
        || url.port().is_some()
        || !(host == "fal.media" || host.ends_with(".fal.media"))
    {
        return Err(FalTransportError::DownloadRejected);
    }
    Ok(url)
}

async fn resolve_public_address(host: &str) -> Result<SocketAddr, FalTransportError> {
    let addresses = tokio::net::lookup_host((host, 443))
        .await
        .map_err(|_| FalTransportError::DownloadRejected)?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public(address.ip())) {
        return Err(FalTransportError::DownloadRejected);
    }
    Ok(addresses[0])
}

fn is_public(value: IpAddr) -> bool {
    match value {
        IpAddr::V4(value) => {
            let [a, b, c, d] = value.octets();
            let reserved = a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 192 && b == 0 && c == 2)
                || (a == 192 && b == 168)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
                || a >= 224
                || [a, b, c, d] == [255, 255, 255, 255];
            !reserved
        }
        IpAddr::V6(value) => {
            if let Some(mapped) = value.to_ipv4_mapped() {
                return is_public(IpAddr::V4(mapped));
            }
            let segments = value.segments();
            !(value.is_loopback()
                || value.is_unicast_link_local()
                || value.is_unique_local()
                || value.is_unspecified()
                || value.is_multicast()
                || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

pub(super) fn status_is_success(status: u16) -> bool {
    StatusCode::from_u16(status).is_ok_and(|value| value.is_success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_download_boundary_rejects_non_fal_and_non_public_targets() {
        assert!(validate_media_url("https://v3b.fal.media/files/image.png").is_ok());
        assert!(validate_media_url("http://v3b.fal.media/files/image.png").is_err());
        assert!(validate_media_url("https://example.com/image.png").is_err());

        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.169.254",
            "192.168.1.1",
            "198.51.100.1",
            "203.0.113.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert!(!is_public(address.parse().unwrap()), "{address}");
        }
        assert!(is_public("8.8.8.8".parse().unwrap()));
        assert!(is_public("2606:4700:4700::1111".parse().unwrap()));
    }
}
