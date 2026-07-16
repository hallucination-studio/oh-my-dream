use std::time::{Duration, Instant};

use tauri::http::{HeaderName, HeaderValue, Request, Response, StatusCode};

use super::{
    DesktopAssetPreviewMethod, DesktopAssetPreviewProtocolRequest,
    DesktopAssetPreviewProtocolResponse,
};

const PREVIEW_READ_DEADLINE: Duration = Duration::from_secs(30);

pub(crate) fn request(value: &Request<Vec<u8>>) -> DesktopAssetPreviewProtocolRequest {
    DesktopAssetPreviewProtocolRequest {
        method: match *value.method() {
            tauri::http::Method::GET => DesktopAssetPreviewMethod::Get,
            tauri::http::Method::HEAD => DesktopAssetPreviewMethod::Head,
            _ => DesktopAssetPreviewMethod::Unsupported,
        },
        uri: canonical_uri(value.uri()),
        range: value
            .headers()
            .get(tauri::http::header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        deadline: Instant::now() + PREVIEW_READ_DEADLINE,
    }
}

fn canonical_uri(uri: &tauri::http::Uri) -> String {
    if matches!(uri.scheme_str(), Some("http" | "https"))
        && uri.host() == Some("desktop-asset.localhost")
    {
        return format!("desktop-asset://{}", uri.path().trim_start_matches('/'));
    }
    uri.to_string()
}

pub(crate) fn response(value: DesktopAssetPreviewProtocolResponse) -> Response<Vec<u8>> {
    let mut response = Response::new(value.body);
    *response.status_mut() =
        StatusCode::from_u16(value.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (name, value) in value.headers {
        let Ok(name) = HeaderName::try_from(name) else {
            continue;
        };
        let Ok(value) = HeaderValue::try_from(value) else {
            continue;
        };
        response.headers_mut().insert(name, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_projection_preserves_method_range_status_headers_and_body() {
        let input = Request::builder()
            .method("GET")
            .uri("desktop-asset://v1/token")
            .header("Range", "bytes=1-2")
            .body(Vec::new())
            .unwrap();
        let projected = request(&input);
        assert_eq!(projected.method, DesktopAssetPreviewMethod::Get);
        assert_eq!(projected.range.as_deref(), Some("bytes=1-2"));

        let output = response(DesktopAssetPreviewProtocolResponse {
            status: 206,
            headers: [("Content-Type".to_owned(), "video/mp4".to_owned())].into(),
            body: vec![1, 2],
        });
        assert_eq!(output.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(output.body(), &[1, 2]);
    }

    #[test]
    fn windows_localhost_transport_normalizes_to_the_canonical_signed_uri() {
        let input = Request::builder()
            .uri("http://desktop-asset.localhost/v1/token")
            .body(Vec::new())
            .unwrap();
        assert_eq!(request(&input).uri, "desktop-asset://v1/token");
    }
}
