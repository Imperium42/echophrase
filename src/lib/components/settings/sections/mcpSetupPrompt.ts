// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Setup prompt for the "Copy setup prompt" button in McpServerSection.
 *
 * This text is meant to be pasted into Claude Code (or another AI agent) so
 * it can wire up the Echophrase CLI as an MCP server for the user's project.
 * It is always English, regardless of the app's UI language, since it is
 * addressed to an AI agent rather than the end user.
 *
 * Source of truth for the `.mcp.json` shape and CLI invocation: cli/README.md
 * (see the "MCP server" section). Keep this in sync if that shape changes.
 */
export const MCP_SETUP_PROMPT = `Set up Echophrase as an MCP server for this project.

1. Create or merge a .mcp.json file in the project root with this server entry:

{
  "mcpServers": {
    "echophrase": {
      "command": "echophrase-cli",
      "args": ["mcp"]
    }
  }
}

If a .mcp.json file already exists, merge the "echophrase" entry into its
existing "mcpServers" object instead of overwriting the file.

2. The "echophrase-cli" command is the Echophrase CLI. It is not bundled with
the desktop app: install it from the Echophrase source with
"cargo install --path cli" (run in the repo root), which puts it on PATH
(running "echophrase-cli --help" should work). If it is not on PATH, use the
absolute path to the "echophrase-cli" binary as the "command" value instead.

3. The Echophrase desktop app must be running, with Settings > MCP Server
enabled, for the "transcribe" tool to work (it talks to the app over a
localhost-only connection so it can reuse the already-loaded model).

4. Once configured, the server exposes 5 tools:
   - record: start recording the microphone
   - stop: stop the active recording and finalize the audio file
   - status: check whether a recording is currently in progress
   - transcode: convert an existing audio file to 16 kHz mono
   - transcribe: transcribe an audio file using the running desktop app's loaded model

Confirm the tools are available after setup.`;
