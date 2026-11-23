# Network and privacy

Echophrase has **no telemetry and no analytics**. It does not report usage, crashes, or
anything else about you or your machine, and there are no accounts.

(This file keeps its old name because many links point at it.)

## The only network calls

| Destination | When | What is sent |
|-------------|------|--------------|
| `huggingface.co` | You download a speech or grammar model (on demand, from Settings or first run) | A normal HTTPS file request for the model you chose. Nothing about your audio or text. |
| `cdn.crabnebula.app` | The app checks for updates, and downloads one if you accept it | The standard Tauri updater request: current app version, OS and CPU architecture. |

Nothing else. Once the models you want are downloaded, Echophrase works fully offline.
`docs/open-source-transition.md` records this policy and what was removed to get here.

## Your audio and text

- Audio is captured in memory and transcribed on your device by the desktop app. It is never
  uploaded, and live capture does not write temporary files.
- The one exception is something you ask for explicitly: the CLI / MCP `record` command writes a
  WAV file to the path you choose (or to the recordings folder if you don't choose one).
- Transcripts stay on your machine. Echophrase has no server to send them to.
- **Restore Clipboard After Pasting** reads the text currently on your clipboard so it can put it
  back after pasting. That happens locally and the text goes nowhere else.

## Older versions

Versions up to and including v0.9.x could send **anonymous** usage stats (OS, app version, GPU
class, crash locations; no user ID, no audio, no text) if you allowed it. That code is gone from
newer builds. Updating removes it, and nothing from older versions is sent after you update.

See also our [Privacy Policy](https://echophrase.com/privacy).
