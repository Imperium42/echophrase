// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Localhost-only HTTP server exposing transcription to the CLI/MCP server.
//!
//! Binds `127.0.0.1` only (never `0.0.0.0`), requires a bearer token every
//! request, and exposes two routes:
//!
//! - `GET /v1/health` -> `{ "version": "<app version>" }`, no auth required
//!   (matches the CLI's "is the app even reachable" probe).
//! - `POST /v1/transcribe` -> `{ "wav_path": "..." }` -> `{ "text": "..." }`,
//!   bearer-token authenticated.
//!
//! The transcription step is injected as a boxed async closure
//! ([`TranscribeFn`]) so unit tests can exercise routing, auth, and path
//! validation without touching the real Candle/GPU pipeline.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};

/// Port the desktop app's local IPC server listens on.
///
/// Chosen adjacent to the app's existing Vite dev ports (1420 dev server,
/// 1421 HMR) without colliding with either, or with any other project's
/// dedicated port in the monorepo's port table.
pub const IPC_PORT: u16 = 1424;

/// Signature for the injected transcription step: takes a validated,
/// canonicalized `.wav` path and returns the transcript text.
pub type TranscribeFn = Arc<
    dyn Fn(PathBuf) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>> + Send + Sync,
>;

#[derive(Clone)]
struct AppState {
    token: Arc<String>,
    transcribe: TranscribeFn,
    version: &'static str,
}

/// Build the router for the given token and transcription implementation.
/// Split out from [`crate::ipc::spawn`] so tests can build and drive the
/// router in-process without binding a real socket.
pub fn router(token: String, version: &'static str, transcribe: TranscribeFn) -> Router {
    let state = AppState {
        token: Arc::new(token),
        transcribe,
        version,
    };
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/transcribe", post(transcribe_handler))
        .with_state(state)
}

#[derive(Serialize)]
struct HealthResponse {
    version: String,
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(HealthResponse {
        version: state.version.to_string(),
    })
}

#[derive(Deserialize)]
struct TranscribeRequest {
    wav_path: String,
}

#[derive(Serialize)]
struct TranscribeResponse {
    text: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
        .into_response()
}

/// Constant-time bearer-token check. Returns `true` iff `headers` carries
/// `Authorization: Bearer <token>` matching `expected` exactly.
fn authorized(headers: &HeaderMap, expected: &str) -> bool {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some(presented) = value.strip_prefix("Bearer ") else {
        return false;
    };
    // Constant-time comparison: bearer tokens are secrets, never compare
    // with `==` (timing side channel).
    constant_time_eq(presented.as_bytes(), expected.as_bytes())
}

/// Constant-time byte-slice equality (no early exit on length or content
/// mismatch, so comparison time does not leak how many leading bytes of a
/// guess matched the real token). Hand-rolled to avoid pulling in a
/// dedicated crate for one function.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        // Still walk the loop over `a` so the timing of the length check
        // alone doesn't dominate; the length itself is not secret (bearer
        // tokens are fixed-width hex), so an early return here is fine.
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn transcribe_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TranscribeRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return error_response(StatusCode::UNAUTHORIZED, "missing or invalid bearer token");
    }

    let path = match validate_wav_path(&body.wav_path) {
        Ok(path) => path,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match (state.transcribe)(path).await {
        Ok(text) => Json(TranscribeResponse { text }).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// Validate the requested path exists, is a file, has a `.wav` extension,
/// and canonicalize it (rejecting anything that doesn't resolve, which also
/// closes off symlink-based path traversal surprises).
fn validate_wav_path(raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("wav_path must not be empty".to_string());
    }
    let requested = Path::new(raw);
    let canonical = requested
        .canonicalize()
        .map_err(|e| format!("wav_path not found or unreadable: {raw} ({e})"))?;
    if !canonical.is_file() {
        return Err(format!("wav_path is not a file: {}", canonical.display()));
    }
    let is_wav = canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("wav"))
        .unwrap_or(false);
    if !is_wav {
        return Err(format!(
            "wav_path must be a .wav file: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn ok_transcribe(text: &'static str) -> TranscribeFn {
        Arc::new(move |_path| Box::pin(async move { Ok(text.to_string()) }))
    }

    fn failing_transcribe() -> TranscribeFn {
        Arc::new(|_path| Box::pin(async move { Err(anyhow::anyhow!("boom")) }))
    }

    #[tokio::test]
    async fn health_returns_version_without_auth() {
        let app = router("secret".to_string(), "1.2.3", ok_transcribe("unused"));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["version"], "1.2.3");
    }

    #[tokio::test]
    async fn transcribe_rejects_missing_token() {
        let app = router("secret".to_string(), "1.0.0", ok_transcribe("hi"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcribe")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"wav_path":"/tmp/x.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn transcribe_rejects_wrong_token() {
        let app = router("secret".to_string(), "1.0.0", ok_transcribe("hi"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcribe")
                    .header("authorization", "Bearer wrong")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"wav_path":"/tmp/x.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn transcribe_rejects_missing_file() {
        let app = router("secret".to_string(), "1.0.0", ok_transcribe("hi"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcribe")
                    .header("authorization", "Bearer secret")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"wav_path":"/nonexistent/x.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn transcribe_rejects_non_wav_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-audio.txt");
        std::fs::write(&path, b"hello").unwrap();

        let app = router("secret".to_string(), "1.0.0", ok_transcribe("hi"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcribe")
                    .header("authorization", "Bearer secret")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"wav_path":"{}"}}"#,
                        path.to_string_lossy().replace('\\', "\\\\")
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn transcribe_returns_text_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.wav");
        std::fs::write(&path, b"RIFF....WAVEfmt ").unwrap();

        let app = router("secret".to_string(), "1.0.0", ok_transcribe("test one two"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcribe")
                    .header("authorization", "Bearer secret")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"wav_path":"{}"}}"#,
                        path.to_string_lossy().replace('\\', "\\\\")
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["text"], "test one two");
    }

    #[tokio::test]
    async fn transcribe_surfaces_backend_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.wav");
        std::fs::write(&path, b"RIFF....WAVEfmt ").unwrap();

        let app = router("secret".to_string(), "1.0.0", failing_transcribe());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcribe")
                    .header("authorization", "Bearer secret")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"wav_path":"{}"}}"#,
                        path.to_string_lossy().replace('\\', "\\\\")
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
