// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase mcp` - a minimal MCP (Model Context Protocol) server over
//! stdio, exposing `record`, `stop`, `status`, `transcode`, and `transcribe`
//! as tools so AI agents can drive voice capture.
//!
//! Every tool delegates to the same [`crate::ops`] functions the CLI
//! subcommands use, so behavior (session bookkeeping, file layout, audio
//! format) never drifts between the two front ends. The hidden `worker`
//! subcommand stays CLI-only; it is an implementation detail of background
//! recording, not something an agent calls directly.

use std::path::PathBuf;

use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ops;

/// Start the stdio MCP server and run until the client disconnects.
pub fn run() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve())
}

async fn serve() -> anyhow::Result<()> {
    let server = McpServer::new();
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

/// Input for the `record` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct RecordInput {
    /// Input device name to capture from. Defaults to the system default.
    device: Option<String>,
    /// Clear a stale session and start fresh instead of erroring.
    #[serde(default)]
    force: bool,
}

/// Input for the `stop` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct StopInput {
    /// Seconds to wait for the background worker to finalize the file.
    timeout_secs: Option<u64>,
}

/// Input for the `transcode` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct TranscodeInput {
    /// Source WAV file to convert.
    input: PathBuf,
    /// Where to write the 16 kHz mono WAV. Defaults to a sibling of `input`
    /// named `<input>.16k.wav`.
    output: Option<PathBuf>,
    /// Peak-normalize the output. Defaults to true.
    #[serde(default = "default_true")]
    normalize: bool,
}

fn default_true() -> bool {
    true
}

/// Input for the `transcribe` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct TranscribeInput {
    /// WAV file to transcribe.
    input: PathBuf,
}

/// The MCP server: a thin tool router over [`crate::ops`].
// `tool_router` is read by the `#[tool_handler]`/`#[tool_router]` macro
// expansion, not by hand-written code, so dead-code analysis misses the use.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct McpServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl McpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Start recording the microphone in the background and return
    /// immediately with the output path and worker pid.
    #[tool(description = "Start recording the microphone in the background.")]
    async fn record(
        &self,
        Parameters(input): Parameters<RecordInput>,
    ) -> Result<Json<ops::RecordStarted>, McpError> {
        ops::start_background_recording(None, input.device, input.force)
            .map(Json)
            .map_err(to_mcp_error)
    }

    /// Stop the active recording and finalize the WAV file.
    #[tool(description = "Stop the active recording and finalize the WAV file.")]
    async fn stop(
        &self,
        Parameters(input): Parameters<StopInput>,
    ) -> Result<Json<ops::StopResult>, McpError> {
        ops::stop(input.timeout_secs.unwrap_or(10))
            .map(Json)
            .map_err(to_mcp_error)
    }

    /// Report whether a recording is currently in progress.
    #[tool(description = "Report whether a recording is currently in progress.")]
    async fn status(&self) -> Result<Json<ops::StatusResult>, McpError> {
        ops::status().map(Json).map_err(to_mcp_error)
    }

    /// Convert an existing WAV file to the canonical 16 kHz mono format.
    #[tool(
        description = "Convert an existing WAV file to 16 kHz mono (downmix + resample + optional peak-normalize)."
    )]
    async fn transcode(
        &self,
        Parameters(input): Parameters<TranscodeInput>,
    ) -> Result<Json<ops::TranscodeResult>, McpError> {
        ops::transcode(&input.input, input.output, input.normalize)
            .map(Json)
            .map_err(to_mcp_error)
    }

    /// Transcribe a WAV file using the running desktop app's loaded model.
    /// Requires the Echophrase desktop app to be running and the app's
    /// "MCP Server" setting to be on. On failure the
    /// error text (from `ops::transcribe`) already distinguishes these
    /// causes - surface it to the user as-is rather than a generic
    /// "start the app" instruction.
    #[tool(
        description = "Transcribe a WAV file using the Echophrase desktop app's loaded transcription model. Requires: (1) the Echophrase desktop app running, (2) the app's Settings > MCP Server toggle is on. If the tool call fails, the error message already says which of these is the actual problem - relay it to the user verbatim rather than assuming it's just 'app not running'."
    )]
    async fn transcribe(
        &self,
        Parameters(input): Parameters<TranscribeInput>,
    ) -> Result<Json<ops::TranscribeResult>, McpError> {
        ops::transcribe(&input.input)
            .map(Json)
            .map_err(to_mcp_error)
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default()
            .with_server_info(Implementation::new("echophrase", env!("CARGO_PKG_VERSION")));
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "Record, stop, check status, and transcode microphone audio for Echophrase. \
             The transcribe tool requires the Echophrase desktop app to be running and \
             the app's Settings > MCP Server toggle to be on - \
             a failed call's error message already says which is missing."
                .to_string(),
        );
        info
    }
}

/// Map an `anyhow::Error` from the ops layer to an MCP internal error,
/// preserving the message so the agent sees why the tool call failed.
fn to_mcp_error(err: anyhow::Error) -> McpError {
    McpError::internal_error(err.to_string(), None)
}
