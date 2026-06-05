//! Axum-compatible gRPC-Web middleware.
//!
//! This is a simplified version of tonic-web's GrpcWebLayer that works with
//! axum's body types. The upstream tonic-web layer only supports tonic's
//! `BoxBody`, which is incompatible with axum's `Router::layer()`.

use axum::body::Body;
use http::{Request, Response, StatusCode, header};
use std::task::{Context, Poll};
use tower::{Layer, Service};

// Re-export for use in this module
use bytes::Bytes;

const GRPC_WEB: &str = "application/grpc-web";
const GRPC_WEB_PROTO: &str = "application/grpc-web+proto";
const GRPC_WEB_TEXT: &str = "application/grpc-web-text";
const GRPC_WEB_TEXT_PROTO: &str = "application/grpc-web-text+proto";
const GRPC_WEB_TRAILERS_BIT: u8 = 0b10000000;

fn is_grpc_web(content_type: Option<&str>) -> bool {
    matches!(
        content_type,
        Some(GRPC_WEB) | Some(GRPC_WEB_PROTO) | Some(GRPC_WEB_TEXT) | Some(GRPC_WEB_TEXT_PROTO)
    )
}

fn is_grpc_web_text(content_type: Option<&str>) -> bool {
    matches!(
        content_type,
        Some(GRPC_WEB_TEXT) | Some(GRPC_WEB_TEXT_PROTO)
    )
}

/// A tower Layer that wraps services with gRPC-Web support for axum.
#[derive(Clone)]
pub struct GrpcWebLayer;

impl GrpcWebLayer {
    pub fn new() -> Self {
        GrpcWebLayer
    }
}

impl<S> Layer<S> for GrpcWebLayer {
    type Service = GrpcWebService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcWebService { inner }
    }
}

/// A tower Service that converts between gRPC-Web and gRPC for axum.
#[derive(Clone)]
pub struct GrpcWebService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for GrpcWebService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());

        // Only process POST requests with gRPC-Web content type
        if req.method() != http::Method::POST || !is_grpc_web(content_type) {
            // Pass through non-gRPC-Web requests unchanged
            let fut = self.inner.call(req);
            return Box::pin(fut);
        }

        let is_text = is_grpc_web_text(content_type);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Decode gRPC-Web request body
            let (parts, body) = req.into_parts();
            let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
                Ok(b) => b,
                Err(_) => {
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::empty())
                        .unwrap());
                }
            };

            let decoded = if is_text {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(&body_bytes) {
                    Ok(d) => Bytes::from(d),
                    Err(_) => {
                        return Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::empty())
                            .unwrap());
                    }
                }
            } else {
                body_bytes
            };

            // Strip gRPC-Web frame header (1 byte flag + 4 byte length)
            let msg_body = if decoded.len() >= 5 {
                decoded.slice(5..)
            } else {
                Bytes::new()
            };

            // Forward as standard gRPC request
            let mut grpc_req = Request::from_parts(parts, Body::from(msg_body));
            grpc_req
                .headers_mut()
                .insert(header::CONTENT_TYPE, "application/grpc".parse().unwrap());
            grpc_req
                .headers_mut()
                .insert(header::TE, "trailers".parse().unwrap());

            let resp = inner.call(grpc_req).await?;

            // Convert gRPC response to gRPC-Web
            let (mut resp_parts, resp_body) = resp.into_parts();
            let resp_bytes = axum::body::to_bytes(resp_body, usize::MAX)
                .await
                .unwrap_or_default();

            // Build gRPC-Web response: message frame + trailer frame
            let mut grpc_web_body = Vec::new();

            // Message frame: 0x00 + 4-byte length + message data
            if !resp_bytes.is_empty() {
                grpc_web_body.push(0x00);
                let len = resp_bytes.len() as u32;
                grpc_web_body.extend_from_slice(&len.to_be_bytes());
                grpc_web_body.extend_from_slice(&resp_bytes);
            }

            // Trailer frame: 0x80 + 4-byte length + trailer data
            // Extract grpc-status from response headers (tonic puts them there for HTTP/1.1)
            let mut trailer_data = Vec::new();
            let status = resp_parts
                .headers
                .remove("grpc-status")
                .map(|v| v.to_str().unwrap_or("0").to_string())
                .unwrap_or_else(|| "0".to_string());
            let message = resp_parts
                .headers
                .remove("grpc-message")
                .map(|v| v.to_str().unwrap_or("").to_string())
                .unwrap_or_default();

            trailer_data.extend_from_slice(b"grpc-status:");
            trailer_data.extend_from_slice(status.as_bytes());
            trailer_data.extend_from_slice(b"\r\n");
            trailer_data.extend_from_slice(b"grpc-message:");
            trailer_data.extend_from_slice(message.as_bytes());
            trailer_data.extend_from_slice(b"\r\n");

            grpc_web_body.push(GRPC_WEB_TRAILERS_BIT);
            let trailer_len = trailer_data.len() as u32;
            grpc_web_body.extend_from_slice(&trailer_len.to_be_bytes());
            grpc_web_body.extend_from_slice(&trailer_data);

            // Set gRPC-Web response headers
            resp_parts
                .headers
                .insert(header::CONTENT_TYPE, GRPC_WEB_PROTO.parse().unwrap());
            // Remove content-length since we're modifying the body
            resp_parts.headers.remove(header::CONTENT_LENGTH);

            Ok(Response::from_parts(resp_parts, Body::from(grpc_web_body)))
        })
    }
}
