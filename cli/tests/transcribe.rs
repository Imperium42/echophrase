// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! End-to-end tests for `echophrase transcribe` and the MCP `transcribe`
//! tool, against a fake local HTTP server standing in for the desktop app's
//! IPC server (real transcription needs a GPU/model - see cli/README.md for
//! the manual QA step against the real app).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use serial_test::serial;
use tempfile::TempDir;

/// Trimmed copy of `tests/mcp.rs`'s stdio JSON-RPC harness, just enough to
/// drive `initialize` + one `tools/call`. Kept local (rather than shared via
/// `include!`) so this file's tests stay independent of that one.
struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpProcess {
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

    fn send(&mut self, message: &Value) {
        let mut line = serde_json::to_string(message).expect("serialize JSON-RPC message");
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .expect("write to child stdin");
        self.stdin.flush().expect("flush child stdin");
    }

    fn recv(&mut self) -> Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read from child stdout");
        assert!(!line.is_empty(), "child closed stdout unexpectedly");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("invalid JSON line {line:?}: {e}"))
    }

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
        assert!(
            response.get("result").is_some(),
            "initialize should succeed: {response}"
        );
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
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
        drop(self.stdin.write_all(b""));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Port `ops::transcribe` (and the real desktop app) use. Kept in sync by
/// hand with `cli/src/ops.rs::IPC_PORT` / `src-tauri/src/ipc/server.rs`.
const IPC_PORT: u16 = 1424;

/// Write the token file the CLI reads, under `<home>/run/ipc-token` - the
/// same layout `paths::ipc_token_file()` resolves to.
fn write_token_file(home: &Path, token: &str) {
    let run_dir = home.join("run");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(run_dir.join("ipc-token"), token).unwrap();
}

/// Write the desktop app's IPC state file the CLI reads on a connection
/// failure, under `<home>/run/ipc-state.json` - the same layout
/// `paths::ipc_state_file()` resolves to.
fn write_state_file(home: &Path, state: &str) {
    let run_dir = home.join("run");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(
        run_dir.join("ipc-state.json"),
        json!({ "state": state }).to_string(),
    )
    .unwrap();
}

/// Accept exactly one HTTP/1.1 request on `IPC_PORT` and reply with a
/// canned `{"text": "..."}` JSON body, regardless of the request contents.
/// Returns a handle to join once the CLI/MCP call under test has completed.
fn serve_one_transcribe_response(text: &'static str) -> std::thread::JoinHandle<()> {
    let listener = TcpListener::bind(("127.0.0.1", IPC_PORT))
        .expect("bind IPC_PORT - is a real echophrase app running on this machine?");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept fake IPC connection");
        read_request(&mut stream);
        let body = json!({ "text": text }).to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    })
}

/// Read a full HTTP/1.1 request (headers + body) off `stream`. We only need
/// this to drain the client's write so it doesn't block on our reply; the
/// fake server doesn't inspect the request.
fn read_request(stream: &mut TcpStream) {
    let mut buf = [0u8; 4096];
    let mut data = Vec::new();
    // Read until the end of the headers.
    let header_end = loop {
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            return;
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    // Drain the body too. Closing a socket with unread data makes macOS send
    // a TCP RST, which the client sees as a failed read of our response.
    let headers = String::from_utf8_lossy(&data[..header_end]).to_ascii_lowercase();
    let content_length = headers
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut remaining = content_length.saturating_sub(data.len() - header_end);
    while remaining > 0 {
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        remaining = remaining.saturating_sub(n);
    }
}

#[test]
#[serial]
fn transcribe_command_prints_transcript_from_running_app() {
    let home = TempDir::new().unwrap();
    write_token_file(home.path(), "test-token");
    let input = home.path().join("in.wav");
    std::fs::write(&input, b"RIFF....WAVEfmt ").unwrap();

    let server = serve_one_transcribe_response("test one two");

    let output = Command::new(env!("CARGO_BIN_EXE_echophrase"))
        .arg("transcribe")
        .arg(&input)
        .env("ECHOPHRASE_HOME", home.path())
        .output()
        .expect("failed to run echophrase transcribe");

    server.join().unwrap();

    assert!(
        output.status.success(),
        "transcribe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "test one two"
    );
}

#[test]
#[serial]
fn transcribe_command_errors_when_app_not_running() {
    let home = TempDir::new().unwrap();
    let input = home.path().join("in.wav");
    std::fs::write(&input, b"RIFF....WAVEfmt ").unwrap();

    // No token file, no fake server: the app is "not running".
    let output = Command::new(env!("CARGO_BIN_EXE_echophrase"))
        .arg("transcribe")
        .arg(&input)
        .env("ECHOPHRASE_HOME", home.path())
        .output()
        .expect("failed to run echophrase transcribe");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("The Echophrase desktop app must be running"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[serial]
fn transcribe_command_errors_with_settings_message_when_mcp_disabled() {
    let home = TempDir::new().unwrap();
    write_state_file(home.path(), "disabled");
    let input = home.path().join("in.wav");
    std::fs::write(&input, b"RIFF....WAVEfmt ").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_echophrase"))
        .arg("transcribe")
        .arg(&input)
        .env("ECHOPHRASE_HOME", home.path())
        .output()
        .expect("failed to run echophrase transcribe");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Settings > MCP Server"), "stderr: {stderr}");
}

#[test]
#[serial]
fn mcp_transcribe_tool_returns_transcript_from_running_app() {
    let home = TempDir::new().unwrap();
    write_token_file(home.path(), "test-token");
    let input = home.path().join("in.wav");
    std::fs::write(&input, b"RIFF....WAVEfmt ").unwrap();

    let server = serve_one_transcribe_response("test one two");

    let mut mcp = McpProcess::spawn(home.path());
    mcp.initialize();

    let response = mcp.call_tool(
        10,
        "transcribe",
        json!({ "input": input.to_string_lossy() }),
    );

    server.join().unwrap();

    assert!(
        response.get("error").is_none(),
        "transcribe tool call errored: {response}"
    );
    let result = &response["result"];
    assert_ne!(
        result.get("isError"),
        Some(&json!(true)),
        "transcribe tool reported failure: {response}"
    );

    let content_text = result["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content");
    let payload: Value = serde_json::from_str(content_text).expect("tool content is JSON");
    assert_eq!(payload["text"], "test one two");
}

#[test]
#[serial]
fn mcp_transcribe_tool_reports_app_not_running() {
    let home = TempDir::new().unwrap();
    let input = home.path().join("in.wav");
    std::fs::write(&input, b"RIFF....WAVEfmt ").unwrap();

    let mut mcp = McpProcess::spawn(home.path());
    mcp.initialize();

    let response = mcp.call_tool(
        11,
        "transcribe",
        json!({ "input": input.to_string_lossy() }),
    );

    // rmcp surfaces tool errors as a JSON-RPC error or an isError result
    // depending on error kind; either way the friendly message must appear
    // somewhere in the response.
    let response_text = response.to_string();
    assert!(
        response_text.contains("The Echophrase desktop app must be running"),
        "response: {response}"
    );
}

#[test]
#[serial]
fn mcp_transcribe_tool_reports_settings_message_when_mcp_disabled() {
    let home = TempDir::new().unwrap();
    write_state_file(home.path(), "disabled");
    let input = home.path().join("in.wav");
    std::fs::write(&input, b"RIFF....WAVEfmt ").unwrap();

    let mut mcp = McpProcess::spawn(home.path());
    mcp.initialize();

    let response = mcp.call_tool(
        13,
        "transcribe",
        json!({ "input": input.to_string_lossy() }),
    );

    let response_text = response.to_string();
    assert!(
        response_text.contains("Settings > MCP Server"),
        "response: {response}"
    );
}
