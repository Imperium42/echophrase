// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Thin Aptabase HTTP client for self-hosted analytics.
//!
//! Replaces the abandoned `tauri-plugin-aptabase` with a direct HTTP client
//! modeled after the official Aptabase Tauri/Swift SDKs (MIT-licensed).
//!
//! Events are queued in memory and flushed in batches:
//! - By the background loop, only once the app has been idle (no transcription)
//!   for IDLE_FLUSH_THRESHOLD - so no request ever fires during or right after
//!   a transcription and a network monitor can never correlate the two
//! - On settings save (immediate flush)
//! - On app exit (blocking flush)

use serde::Serialize;
use std::time::Instant;

/// Session timeout: rotate session ID after 1 hour of inactivity (matches official SDKs).
const SESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3600);

/// Max events per HTTP request (matches Aptabase Tauri SDK).
const BATCH_SIZE: usize = 25;

/// Flush interval: 60s in production, 2s in debug (matches Aptabase Tauri/Swift SDKs).
#[cfg(debug_assertions)]
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(not(debug_assertions))]
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// The background loop only flushes after this much time with no transcription
/// activity, so telemetry never leaves the machine during or right after a
/// transcription (privacy optics: a POST seconds after speech looks like the
/// transcript being uploaded, even though events are a few hundred bytes of
/// metadata). Exit and settings-save flushes bypass this gate.
#[cfg(debug_assertions)]
const IDLE_FLUSH_THRESHOLD: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(not(debug_assertions))]
const IDLE_FLUSH_THRESHOLD: std::time::Duration = std::time::Duration::from_secs(600);

/// System properties sent with every event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemProps {
    is_debug: bool,
    locale: String,
    os_name: String,
    os_version: String,
    app_version: String,
    sdk_version: String,
}

/// Event payload matching the Aptabase `/api/v0/event` schema.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventPayload {
    timestamp: String,
    session_id: String,
    event_name: String,
    system_props: SystemProps,
    #[serde(skip_serializing_if = "Option::is_none")]
    props: Option<serde_json::Value>,
}

/// Lightweight Aptabase client that queues events and flushes them in batches.
pub struct AptabaseClient {
    http: reqwest::Client,
    app_key: String,
    host: String,
    session_id: parking_lot::Mutex<SessionState>,
    queue: parking_lot::Mutex<Vec<EventPayload>>,
    system_props: SystemProps,
    last_activity: parking_lot::Mutex<Option<Instant>>,
}

/// Mutable session state protected by a mutex.
struct SessionState {
    id: String,
    last_touched: Instant,
}

impl AptabaseClient {
    /// Create a new client. Call once during app setup.
    pub fn new(app_key: &str, host: &str, app_version: &str) -> Self {
        let session_id = generate_session_id();

        // A User-Agent is required: Cloudflare serves a managed challenge to
        // UA-less requests, silently eating every event (reqwest sends none by default).
        let http = reqwest::Client::builder()
            .user_agent(format!("echophrase/{}", app_version))
            .build()
            .unwrap_or_default();

        Self {
            http,
            app_key: app_key.to_string(),
            host: host.trim_end_matches('/').to_string(),
            session_id: parking_lot::Mutex::new(SessionState {
                id: session_id,
                last_touched: Instant::now(),
            }),
            queue: parking_lot::Mutex::new(Vec::new()),
            last_activity: parking_lot::Mutex::new(None),
            system_props: SystemProps {
                is_debug: cfg!(debug_assertions),
                locale: sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string()),
                os_name: detect_os_name(),
                os_version: get_os_version(),
                app_version: app_version.to_string(),
                sdk_version: format!("echophrase-rust/{}", app_version),
            },
        }
    }

    /// Queue an event for batched delivery. Does not make any HTTP calls.
    pub fn track_event(&self, event_name: &str, props: Option<serde_json::Value>) {
        let payload = EventPayload {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: self.current_session_id(),
            event_name: event_name.to_string(),
            system_props: self.system_props.clone(),
            props,
        };

        self.queue.lock().push(payload);
    }

    /// Flush all queued events asynchronously. Spawns a task and returns immediately.
    pub fn flush(&self) {
        let events = self.drain_queue();
        // Empty key = official build compiled without APTABASE_APP_KEY (build.rs
        // warns). Drop the events rather than send requests Aptabase rejects.
        if events.is_empty() || self.app_key.is_empty() {
            return;
        }

        let client = self.http.clone();
        let url = format!("{}/api/v0/event", self.host);
        let app_key = self.app_key.clone();

        tauri::async_runtime::spawn(async move {
            send_events(&client, &url, &app_key, events).await;
        });
    }

    /// Flush all queued events synchronously. Blocks until complete.
    /// Use only on app exit where we cannot rely on async tasks completing.
    pub fn flush_blocking(&self) {
        let events = self.drain_queue();
        // Empty key = official build compiled without APTABASE_APP_KEY (build.rs
        // warns). Drop the events rather than send requests Aptabase rejects.
        if events.is_empty() || self.app_key.is_empty() {
            return;
        }

        let client = self.http.clone();
        let url = format!("{}/api/v0/event", self.host);
        let app_key = self.app_key.clone();

        // Block on the async flush using tauri's runtime
        tauri::async_runtime::block_on(async move {
            send_events(&client, &url, &app_key, events).await;
        });
    }

    /// Mark transcription activity, deferring background flushes until the app
    /// has been idle for IDLE_FLUSH_THRESHOLD. Call when a recording starts and
    /// when a transcription completes.
    pub fn mark_activity(&self) {
        *self.last_activity.lock() = Some(Instant::now());
    }

    /// Whether the app has been idle (no transcription activity) long enough
    /// for the background loop to flush. Sessions with no activity at all
    /// count as idle.
    fn is_idle(&self) -> bool {
        self.last_activity
            .lock()
            .map(|t| t.elapsed() >= IDLE_FLUSH_THRESHOLD)
            .unwrap_or(true)
    }

    /// Start the background flush loop. Call once after creation.
    ///
    /// The loop only sends while the app is idle - see IDLE_FLUSH_THRESHOLD.
    /// Queued events are never dropped by the gate; they simply ride a later
    /// flush (or the exit flush).
    pub fn start_flush_loop(&'static self) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(FLUSH_INTERVAL).await;
                if self.is_idle() {
                    self.flush();
                }
            }
        });
    }

    /// Drain all events from the queue, returning them.
    fn drain_queue(&self) -> Vec<EventPayload> {
        let mut queue = self.queue.lock();
        std::mem::take(&mut *queue)
    }

    /// Get the current session ID, rotating if the session has timed out.
    fn current_session_id(&self) -> String {
        let mut state = self.session_id.lock();
        if state.last_touched.elapsed() > SESSION_TIMEOUT {
            state.id = generate_session_id();
        }
        state.last_touched = Instant::now();
        state.id.clone()
    }
}

/// Send events in batches of BATCH_SIZE via individual POSTs.
/// (Aptabase self-hosted only supports `/api/v0/event` singular, not batch endpoint.)
async fn send_events(
    client: &reqwest::Client,
    url: &str,
    app_key: &str,
    events: Vec<EventPayload>,
) {
    for batch in events.chunks(BATCH_SIZE) {
        for event in batch {
            let result = client
                .post(url)
                .header("App-Key", app_key)
                .json(event)
                .send()
                .await;

            if let Err(e) = result {
                log::debug!("Aptabase event '{}' failed: {}", event.event_name, e);
            }
        }
    }
}

/// Generate a session ID matching the official SDK format:
/// `"{unix_epoch_secs}{8-digit-zero-padded-random}"`.
fn generate_session_id() -> String {
    use rand::Rng;
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let random_part: u32 = rand::thread_rng().gen_range(0..100_000_000);
    format!("{}{:08}", epoch_secs, random_part)
}

/// OS name for the dashboard's per-OS breakdown. WSL reports as its own
/// top-level name ("Linux (WSL)") rather than plain "Linux": the runtime
/// environment (GPU passthrough, audio routing, paste behavior) differs enough
/// that lumping the two together hides real signal.
fn detect_os_name() -> String {
    #[cfg(target_os = "linux")]
    {
        if std::fs::read_to_string("/proc/version")
            .map(|v| v.to_lowercase().contains("microsoft"))
            .unwrap_or(false)
        {
            return "Linux (WSL)".to_string();
        }
    }
    capitalize_os(std::env::consts::OS)
}

/// Distro flavor from /etc/os-release, e.g. "Ubuntu 24.04" or "Arch Linux".
/// Prefers NAME + VERSION_ID over PRETTY_NAME to keep values short and
/// stable (PRETTY_NAME can include point releases and codenames, which
/// would fragment the dashboard breakdown).
#[cfg(target_os = "linux")]
fn detect_linux_distro() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/os-release").ok()?;
    let field = |key: &str| -> Option<String> {
        contents
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty())
    };

    let name = field("NAME=")?;
    match field("VERSION_ID=") {
        // Rolling-release distros (Arch, openSUSE Tumbleweed) have no VERSION_ID.
        None => Some(name),
        Some(version) => Some(format!("{} {}", name, version)),
    }
}

/// Capitalize OS name for display (e.g. "linux" -> "Linux").
fn capitalize_os(os: &str) -> String {
    let mut chars = os.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Best-effort OS version detection.
fn get_os_version() -> String {
    // On Linux, "version" is the distro flavor (e.g. "Ubuntu 24.04"), not the
    // kernel version: it feeds the os_version child breakdown under the
    // "Linux" / "Linux (WSL)" os_name parents in the Aptabase dashboard.
    #[cfg(target_os = "linux")]
    {
        if let Some(distro) = detect_linux_distro() {
            return distro;
        }
        if let Ok(contents) = std::fs::read_to_string("/proc/version") {
            if let Some(version_part) = contents.split_whitespace().nth(2) {
                return version_part.to_string();
            }
        }
    }

    // Major version only (e.g. "macOS 26"): point releases would fragment
    // the os_version child breakdown in the dashboard.
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(major) = version.split('.').next().filter(|m| !m.is_empty()) {
                return format!("macOS {}", major);
            }
        }
    }

    // Major product name only ("Windows 11" / "Windows 10"): both report
    // kernel 10.0, so the build number is the real discriminator
    // (>= 22000 means Windows 11).
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
        {
            // "Microsoft Windows [Version 10.0.22631.4460]"
            let raw = String::from_utf8_lossy(&output.stdout);
            if let Some(build) = raw
                .split("Version ")
                .nth(1)
                .and_then(|v| v.split('.').nth(2))
                .and_then(|b| b.trim().parse::<u32>().ok())
            {
                return if build >= 22000 {
                    "Windows 11".to_string()
                } else {
                    "Windows 10".to_string()
                };
            }
            let version = raw.trim().to_string();
            if !version.is_empty() {
                return version;
            }
        }
    }

    "unknown".to_string()
}
