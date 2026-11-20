// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! End-to-end tests for `echophrase mcp`, driving the real compiled binary
//! headless over stdio: a JSON-RPC `initialize` handshake, `tools/list`, and
//! a `tools/call` on the `transcode` tool against a fixture WAV.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use hound::{SampleFormat, WavSpec, WavWriter};
use serde_json::{Value, json};
use serial_test::serial;
use tempfile::TempDir;

/// A running `echophrase mcp` process plus its stdio handles.
struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpProcess {
    /// Spawn `echophrase mcp` with `ECHOPHRASE_HOME` pointed at an isolated
    /// temp dir so this test never touches real session state.
    fn spawn(home: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_echophrase"))
            .arg("mcp")
            .env("ECHOPHRASE_HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn echophrase mcp");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    /// Send a single newline-delimited JSON-RPC message (request or
    /// notification) - the framing rmcp's stdio transport expects.
    fn send(&mut self, message: &Value) {
        let mut line = serde_json::to_string(message).expect("serialize JSON-RPC message");
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .expect("write to child stdin");
        self.stdin.flush().expect("flush child stdin");
    }

    /// Read and parse one newline-delimited JSON-RPC response line.
    fn recv(&mut self) -> Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read from child stdout");
        assert!(!line.is_empty(), "child closed stdout unexpectedly");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("invalid JSON line {line:?}: {e}"))
    }

    /// Full handshake: `initialize` request/response, then the
    /// `notifications/initialized` notification.
    fn initialize(&mut self) {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "echophrase-cli-tests", "version": "0.0.0" }
            }
        }));
        let response = self.recv();
        assert_eq!(response["id"], json!(1));
        assert!(
            response.get("result").is_some(),
            "initialize should succeed: {response}"
        );

        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    fn tools_list(&mut self) -> Value {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }));
        self.recv()
    }

    fn call_tool(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }));
        self.recv()
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        // Closing stdin lets the server's stdio transport see EOF and exit
        // cleanly; kill is a backstop if it doesn't.
        drop(self.stdin.write_all(b""));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Write a short mono 48 kHz, 16-bit WAV fixture.
fn write_fixture_wav(path: &Path) {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec).unwrap();
    for i in 0..48_000 {
        let t = i as f32 / 48_000.0;
        let v = (0.3 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 32767.0) as i16;
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
#[serial]
fn initialize_and_tools_list_returns_four_tools() {
    let home = TempDir::new().unwrap();
    let mut mcp = McpProcess::spawn(home.path());
    mcp.initialize();

    let response = mcp.tools_list();
    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools/list should return a tools array");

    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool name"))
        .collect();

    assert_eq!(names.len(), 5, "expected exactly 5 tools, got {names:?}");
    for expected in ["record", "stop", "status", "transcode", "transcribe"] {
        assert!(
            names.contains(&expected),
            "tools/list missing `{expected}`: {names:?}"
        );
    }
    // The hidden `worker` subcommand must never be exposed as an MCP tool.
    assert!(!names.contains(&"worker"));
}

#[test]
#[serial]
fn transcode_tool_writes_16k_mono_output() {
    let home = TempDir::new().unwrap();
    let input = home.path().join("in.wav");
    write_fixture_wav(&input);
    let output = home.path().join("out.wav");

    let mut mcp = McpProcess::spawn(home.path());
    mcp.initialize();

    let response = mcp.call_tool(
        3,
        "transcode",
        json!({
            "input": input.to_string_lossy(),
            "output": output.to_string_lossy(),
            "normalize": true
        }),
    );

    assert!(
        response.get("error").is_none(),
        "transcode tool call errored: {response}"
    );
    let result = &response["result"];
    assert_ne!(
        result.get("isError"),
        Some(&json!(true)),
        "transcode tool reported failure: {response}"
    );

    assert!(output.exists(), "transcode did not write {output:?}");
    let reader = hound::WavReader::open(&output).expect("output WAV should exist");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000);
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.bits_per_sample, 16);

    // The structured JSON payload should also be present in the tool result
    // content and describe the same output path.
    let content_text = result["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content");
    let payload: Value = serde_json::from_str(content_text).expect("tool content is JSON");
    assert_eq!(
        payload["output_path"].as_str().map(Path::new),
        Some(output.as_path())
    );
    assert_eq!(payload["sample_rate"], json!(16_000));
}

#[test]
#[serial]
fn status_tool_reports_idle_with_no_session() {
    let home = TempDir::new().unwrap();
    let mut mcp = McpProcess::spawn(home.path());
    mcp.initialize();

    let response = mcp.call_tool(4, "status", json!({}));
    assert!(
        response.get("error").is_none(),
        "status errored: {response}"
    );

    let content_text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content");
    let payload: Value = serde_json::from_str(content_text).expect("tool content is JSON");
    assert_eq!(payload["recording"], json!(false));
    assert!(payload["session"].is_null());
}
