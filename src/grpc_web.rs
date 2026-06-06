//! Axum-compatible gRPC-Web middleware.
//!
//! This is a simplified version of tonic-web's GrpcWebLayer that works with
//! axum's body types. The upstream tonic-web layer only supports tonic's
//! `BoxBody`, which is incompatible with axum's `Router::layer()`.

use axum::body::Body;
use http::{Request, Response, header};
use std::task::{Context, Poll};
use tower::{Layer, Service};

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
///
/// For requests: rewrites Content-Type from gRPC-Web to gRPC and adds TE header.
/// The message frame format is identical between gRPC and gRPC-Web, so the body
/// passes through unchanged — tonic's decoder handles the framing.
///
/// For responses: extracts grpc-status/grpc-message from HTTP/1.1 headers (where
/// tonic puts them) and encodes them as a gRPC-Web trailer frame in the body.
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

        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Rewrite headers: gRPC-Web → gRPC (body format is the same)
            let (mut parts, body) = req.into_parts();
            parts
                .headers
                .insert(header::CONTENT_TYPE, "application/grpc".parse().unwrap());
            parts
                .headers
                .insert(header::TE, "trailers".parse().unwrap());

            let grpc_req = Request::from_parts(parts, body);
            let resp = inner.call(grpc_req).await?;

            // Convert gRPC response to gRPC-Web: encode trailers in body
            let (mut resp_parts, resp_body) = resp.into_parts();
            let resp_bytes = axum::body::to_bytes(resp_body, usize::MAX)
                .await
                .unwrap_or_default();

            let mut grpc_web_body = Vec::new();

            // Message frame: 0x00 + 4-byte length + message data
            if !resp_bytes.is_empty() {
                // Tonic may already wrap the response in gRPC length-prefixed
                // framing (0x00 + 4-byte BE length + message). Detect and
                // extract the inner message to avoid double-wrapping, which
                // causes ConnectRPC's parser to see a stray 0x00 byte and fail
                // with "illegal tag: field no 0 wire type 0".
                let message_data =
                    if resp_bytes.len() >= 5 && (resp_bytes[0] == 0x00 || resp_bytes[0] == 0x01) {
                        let msg_len = u32::from_be_bytes([
                            resp_bytes[1],
                            resp_bytes[2],
                            resp_bytes[3],
                            resp_bytes[4],
                        ]) as usize;
                        if 5 + msg_len == resp_bytes.len() {
                            // Already framed — use the inner message directly
                            &resp_bytes[5..]
                        } else {
                            // Length mismatch — treat as raw protobuf
                            &resp_bytes
                        }
                    } else {
                        &resp_bytes
                    };

                grpc_web_body.push(0x00);
                let len = message_data.len() as u32;
                grpc_web_body.extend_from_slice(&len.to_be_bytes());
                grpc_web_body.extend_from_slice(message_data);
            }

            // Trailer frame: 0x80 + 4-byte length + trailer data
            // Tonic sends grpc-status/grpc-message as HTTP/1.1 headers
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
            resp_parts.headers.remove(header::CONTENT_LENGTH);

            Ok(Response::from_parts(resp_parts, Body::from(grpc_web_body)))
        })
    }
}
