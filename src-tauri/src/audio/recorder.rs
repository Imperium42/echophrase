// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Cross-platform audio recorder built on `echo_audio::capture` (cpal).
//!
//! One capture backend for every desktop platform (WASAPI / CoreAudio / ALSA);
//! the old PulseAudio (WSL) and JS mic-recorder plugin (macOS/Windows) paths
//! are gone. Audio arrives as 16 kHz mono f32 chunks over a channel and is
//! buffered IN MEMORY - no temp files anywhere in the live-recording path.
//!
//! Two session kinds share the same plumbing:
//!
//! * **Plain recording** ([`AudioRecorder::start_recording`]): chunks append to
//!   a buffer until [`AudioRecorder::stop_recording_raw`] returns the samples.
//! * **Open mic** ([`AudioRecorder::start_recording_with_vad`]): each 256-sample
//!   chunk (Silero VAD v5 requirement) runs through the speech state machine;
//!   completed utterances land in `pending_queue` for
//!   [`AudioRecorder::take_pending_buffer`].
//!
//! Locking discipline (inherited from the previous PulseAudio implementation):
//! the consumer task owns the channel receiver and re-acquires the recorder
//! lock only briefly per chunk, so stop/reset/take_pending never wait on I/O.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use echo_audio::capture::{CaptureConfig, CaptureEvent, CaptureHandle, start_capture};
use echo_audio::sample;

use crate::vad::VadBackend;

/// Enable verbose logging for Open Mic VAD loop debugging.
/// On in debug builds (`bun run tauri:dev`), off in release.
const OPEN_MIC_DEBUG: bool = cfg!(debug_assertions);

/// Chunk size for plain recording: 64ms at 16 kHz. Small enough for a prompt
/// stop, large enough to keep channel traffic negligible.
const RECORD_CHUNK_SAMPLES: usize = 1024;

/// Chunk size for open mic: Silero VAD v5 requires exactly 256 samples (16ms
/// at 16 kHz). Larger chunks corrupt the ONNX LSTM state.
const VAD_CHUNK_SAMPLES: usize = 256;

/// Maximum number of completed speech segments held in `pending_queue` before
/// dropping the oldest. Bounds memory growth if the frontend stops draining
/// chunks (feature-gate block, JS error). At ~5s/chunk this is ~25s of audio.
const PENDING_QUEUE_MAX: usize = 5;

/// Minimum speech duration before a segment counts as real speech (not noise).
const MIN_SPEECH_DURATION_MS: u64 = 500;

/// Periodic VAD reset during idle: prevents Silero LSTM drift from processing
/// amplified noise over long silences. 62 frames * 16ms = ~1 second.
const VAD_IDLE_RESET_INTERVAL: u64 = 62;

/// Idle pre-speech cushion kept in the buffer (~192ms) so the first word of an
/// utterance is not clipped.
const PRE_SPEECH_SAMPLES: usize = 12 * VAD_CHUNK_SAMPLES;

/// Frames quieter than this are left un-amplified for VAD (noise floor).
const VAD_NORMALIZE_MIN_PEAK: f32 = 0.002;

/// Duration in ms from `speech_start` (the segment's first speech onset) up to
/// `at`. Returns 0 if the segment never started (nothing to measure).
///
/// `speech_start` is set exactly once, at `WaitingForSpeech -> Speaking`, and
/// is NOT reset by a `SilenceAfterSpeech -> Speaking` bounce - so this always
/// reflects the whole segment's accumulated speech time, not just the time
/// since the most recent resume.
fn segment_duration_ms(speech_start: Option<Instant>, at: Instant) -> u64 {
    speech_start
        .map(|t| at.saturating_duration_since(t).as_millis() as u64)
        .unwrap_or(0)
}

/// Whether a speech segment should be discarded as a false VAD trigger: its
/// total accumulated speech duration (since first onset) is below
/// `MIN_SPEECH_DURATION_MS`. A segment that bounced through
/// `SilenceAfterSpeech` and back to `Speaking` one or more times is NOT a
/// false trigger by this measure, as long as its total speech time cleared
/// the threshold - only a genuinely brief blip (that never accumulated
/// enough speech) is discarded.
fn is_false_trigger(speech_start: Option<Instant>, at: Instant) -> bool {
    segment_duration_ms(speech_start, at) < MIN_SPEECH_DURATION_MS
}

/// Speech detection state machine for open mic mode
#[derive(Debug, Clone, PartialEq)]
pub enum SpeechState {
    /// Waiting for speech to begin
    WaitingForSpeech,
    /// Speech detected, recording actively
    Speaking,
    /// Speech ended, counting silence duration before triggering transcription
    SilenceAfterSpeech,
}

/// Mutable state shared between commands and the chunk-consumer task.
struct RecorderInner {
    is_recording: bool,
    /// Live capture session; dropping it stops the cpal stream.
    session: Option<CaptureHandle>,
    /// Completion signal: the consumer task drops its sender when it has
    /// drained the final chunks, so `stop_recording_raw` can await a race-free
    /// buffer handoff.
    done_rx: Option<tokio::sync::oneshot::Receiver<()>>,
    /// Accumulated audio: 16 kHz mono f32.
    buffer: Vec<f32>,
    // ---- open mic (VAD) state machine ----
    vad_enabled: bool,
    speech_state: SpeechState,
    silence_start: Option<Instant>,
    silence_notified: bool,
    speech_start: Option<Instant>,
    silence_timeout_ms: u64,
    vad_threshold: f32,
    /// Sample offset in `buffer` where speech audio begins (excludes leading silence).
    speech_start_offset: usize,
    /// Queue of completed speech segments ready for transcription.
    pending_queue: VecDeque<Vec<f32>>,
}

impl RecorderInner {
    fn new() -> Self {
        Self {
            is_recording: false,
            session: None,
            done_rx: None,
            buffer: Vec::new(),
            vad_enabled: false,
            speech_state: SpeechState::WaitingForSpeech,
            silence_start: None,
            silence_notified: false,
            speech_start: None,
            silence_timeout_ms: 5000,
            vad_threshold: 0.5,
            speech_start_offset: 0,
            pending_queue: VecDeque::new(),
        }
    }

    /// Reset every field to idle. Dropping `session` stops the cpal stream.
    fn reset(&mut self) {
        self.is_recording = false;
        self.vad_enabled = false;
        self.session = None;
        self.done_rx = None;
        self.buffer.clear();
        self.pending_queue.clear();
        self.speech_state = SpeechState::WaitingForSpeech;
        self.silence_start = None;
        self.silence_notified = false;
        self.speech_start = None;
        self.speech_start_offset = 0;
    }
}

pub struct AudioRecorder {
    inner: Arc<Mutex<RecorderInner>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        log::debug!("Audio recorder initialized (echo-audio/cpal, all platforms)");
        Self {
            inner: Arc::new(Mutex::new(RecorderInner::new())),
        }
    }

    /// Start a plain recording session (chunks buffer until stop).
    pub async fn start_recording(&self, app: tauri::AppHandle) -> Result<(), String> {
        ensure_mic_permission().await?;
        let mut inner = self.inner.lock().await;
        if inner.is_recording {
            return Err("Already recording".to_string());
        }

        let (handle, events) = open_capture(RECORD_CHUNK_SAMPLES)?;
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        inner.buffer.clear();
        inner.is_recording = true;
        inner.vad_enabled = false;
        inner.session = Some(handle);
        inner.done_rx = Some(done_rx);
        drop(inner);

        let inner_arc = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            run_record_loop(events, inner_arc, app);
            drop(done_tx); // completion signal for stop_recording_raw
        });

        Ok(())
    }

    /// Stop the active session and return the captured 16 kHz mono samples.
    ///
    /// In open mic mode, queued utterances plus any in-progress speech are
    /// merged so nothing said before the stop is lost.
    pub async fn stop_recording_raw(&self) -> Result<Vec<f32>, String> {
        let done_rx = {
            let mut inner = self.inner.lock().await;
            if !inner.is_recording {
                return Err("Not recording".to_string());
            }
            // Flag first, then stop the capture stream: the consumer task sees
            // the flag, drains the tail chunks, and drops its done sender.
            inner.is_recording = false;
            inner.vad_enabled = false;
            if let Some(session) = inner.session.take() {
                session.stop(); // joins the capture thread; tail chunk flushed
            }
            inner.done_rx.take()
        };

        // Wait for the consumer task to finish draining (either Ok or the
        // sender being dropped both mean "done").
        if let Some(rx) = done_rx {
            let _ = rx.await;
        }

        let mut inner = self.inner.lock().await;
        let samples = collect_session_samples(&mut inner);
        inner.reset();
        Ok(samples)
    }

    /// Start recording with inline VAD for open mic mode.
    /// VAD runs on each 16ms chunk to detect speech/silence transitions. When
    /// silence exceeds the timeout after speech, the utterance moves to the
    /// pending queue and `open-mic-chunk-ready` is emitted. Recording continues.
    pub async fn start_recording_with_vad(
        &self,
        app: tauri::AppHandle,
        threshold: f32,
        silence_timeout_ms: u64,
    ) -> Result<(), String> {
        ensure_mic_permission().await?;
        let mut inner = self.inner.lock().await;
        if inner.is_recording {
            return Err("Already recording".to_string());
        }

        // Create the VAD before touching any state so a failure leaves us idle.
        let vad =
            crate::vad::SileroVad::new().map_err(|e| format!("Failed to create VAD: {}", e))?;

        let (handle, events) = open_capture(VAD_CHUNK_SAMPLES)?;
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        inner.buffer.clear();
        inner.pending_queue.clear();
        inner.is_recording = true;
        inner.vad_enabled = true;
        inner.speech_state = SpeechState::WaitingForSpeech;
        inner.silence_start = None;
        inner.silence_notified = false;
        inner.speech_start = None;
        inner.silence_timeout_ms = silence_timeout_ms;
        inner.vad_threshold = threshold;
        inner.speech_start_offset = 0;
        inner.session = Some(handle);
        inner.done_rx = Some(done_rx);
        drop(inner);

        let inner_arc = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            run_open_mic_loop(events, inner_arc, app, vad, threshold);
            drop(done_tx);
        });

        Ok(())
    }

    /// Take the next completed open-mic utterance (16 kHz mono f32), if any.
    pub async fn take_pending_buffer(&self) -> Result<Option<Vec<f32>>, String> {
        let mut inner = self.inner.lock().await;
        Ok(inner.pending_queue.pop_front())
    }

    /// Force reset recording state - use to recover from stuck "Already
    /// recording" errors. Safe to call even if not recording.
    pub async fn reset_recording_state(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        inner.reset();
        log::debug!("Recording state reset - ready for new recording");
        Ok(())
    }
}

/// Ensure microphone permission before opening a capture stream.
///
/// macOS: triggers the TCC prompt via AVFoundation on first use - without this
/// pre-request, cpal/CoreAudio can silently deliver no devices when the user
/// has not yet decided (Info.plist NSMicrophoneUsageDescription supplies the
/// prompt text). Other platforms need nothing (WASAPI/ALSA prompt-free).
#[cfg(target_os = "macos")]
async fn ensure_mic_permission() -> Result<(), String> {
    use crate::macos_permissions::{MicAuthStatus, request_access_blocking};
    let status = tokio::task::spawn_blocking(request_access_blocking)
        .await
        .map_err(|e| format!("Permission check failed: {e}"))?;
    match status {
        MicAuthStatus::Authorized => Ok(()),
        other => Err(format!(
            "Microphone access {}. Enable it in System Settings > Privacy & Security > Microphone.",
            other.as_str()
        )),
    }
}

#[cfg(not(target_os = "macos"))]
async fn ensure_mic_permission() -> Result<(), String> {
    Ok(())
}

/// Open a capture session on the user-selected input device, falling back to
/// the system default. The persisted `input_device` is a Pulse source name on
/// WSL and a cpal device display name on Windows/macOS; `"default"` (or unset)
/// means "system default", which `echo_audio::capture` maps to `device_name:
/// None` on every platform.
fn open_capture(
    chunk_samples: usize,
) -> Result<(CaptureHandle, std::sync::mpsc::Receiver<CaptureEvent>), String> {
    let device_name = crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| s.input_device)
        .filter(|id| !id.is_empty() && id != "default");

    start_capture(CaptureConfig {
        device_name,
        chunk_samples,
    })
    .map_err(|e| format!("Failed to start audio capture: {e}"))
}

/// Merge everything captured this session into one sample vector: queued
/// utterances first, then in-progress speech (or the whole buffer when VAD
/// never split it).
fn collect_session_samples(inner: &mut RecorderInner) -> Vec<f32> {
    if !inner.pending_queue.is_empty() {
        let mut combined: Vec<f32> = Vec::new();
        for pending in inner.pending_queue.drain(..) {
            combined.extend_from_slice(&pending);
        }
        // Include any in-progress speech from the current buffer.
        if inner.speech_start_offset < inner.buffer.len() {
            combined.extend_from_slice(&inner.buffer[inner.speech_start_offset..]);
        }
        combined
    } else if inner.speech_state == SpeechState::Speaking
        || inner.speech_state == SpeechState::SilenceAfterSpeech
    {
        // Mid-speech stop: only take audio from where speech started.
        inner.buffer[inner.speech_start_offset..].to_vec()
    } else {
        std::mem::take(&mut inner.buffer)
    }
}

/// Plain recording consumer: append chunks under a brief lock until stopped.
fn run_record_loop(
    events: std::sync::mpsc::Receiver<CaptureEvent>,
    inner_arc: Arc<Mutex<RecorderInner>>,
    app: tauri::AppHandle,
) {
    loop {
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(CaptureEvent::Chunk(chunk)) => {
                let mut inner = inner_arc.blocking_lock();
                if !inner.is_recording {
                    // Stopping: append this and everything already queued so
                    // the tail is never lost, then finish.
                    inner.buffer.extend_from_slice(&chunk);
                    drain_remaining_chunks(&events, &mut inner.buffer);
                    break;
                }
                inner.buffer.extend_from_slice(&chunk);
            }
            Ok(CaptureEvent::Error(e)) => {
                log::warn!("⚠️ Error reading audio: {e}");
                log::warn!("🔄 Auto-resetting recording state due to error");
                inner_arc.blocking_lock().reset();
                crate::tray_status::set(&app, crate::tray_status::TrayStatus::Idle);
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No audio this tick; exit if stop was requested meanwhile.
                let mut inner = inner_arc.blocking_lock();
                if !inner.is_recording {
                    drain_remaining_chunks(&events, &mut inner.buffer);
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Capture thread gone (session stopped/dropped): drain and exit.
                let mut inner = inner_arc.blocking_lock();
                drain_remaining_chunks(&events, &mut inner.buffer);
                break;
            }
        }
    }
}

/// Append every chunk still sitting in the channel to `buffer`.
fn drain_remaining_chunks(events: &std::sync::mpsc::Receiver<CaptureEvent>, buffer: &mut Vec<f32>) {
    for event in events.try_iter() {
        if let CaptureEvent::Chunk(chunk) = event {
            buffer.extend_from_slice(&chunk);
        }
    }
}

/// Open-mic consumer: run each 256-sample chunk through Silero VAD and the
/// speech state machine. Ported 1:1 from the PulseAudio loop, with byte
/// offsets replaced by sample offsets (chunks arrive as 16 kHz mono f32).
fn run_open_mic_loop(
    events: std::sync::mpsc::Receiver<CaptureEvent>,
    inner_arc: Arc<Mutex<RecorderInner>>,
    app: tauri::AppHandle,
    mut vad: crate::vad::SileroVad,
    threshold: f32,
) {
    use tauri::Emitter;

    let mut debug_frame_count: u64 = 0;
    let mut idle_silence_frames: u64 = 0;

    if OPEN_MIC_DEBUG {
        log::debug!("[OpenMic] VAD loop started, threshold={threshold:.3}");
    }

    loop {
        let chunk = match events.recv_timeout(Duration::from_millis(100)) {
            Ok(CaptureEvent::Chunk(chunk)) => chunk,
            Ok(CaptureEvent::Error(e)) => {
                log::warn!("Error reading audio (open mic): {e}");
                inner_arc.blocking_lock().reset();
                crate::tray_status::set(&app, crate::tray_status::TrayStatus::Idle);
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let inner = inner_arc.blocking_lock();
                if !inner.is_recording || !inner.vad_enabled {
                    if OPEN_MIC_DEBUG {
                        log::debug!(
                            "[OpenMic] Loop exit: recording={} vad={}",
                            inner.is_recording,
                            inner.vad_enabled
                        );
                    }
                    break;
                }
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };

        // Event flags, emitted OUTSIDE the lock to avoid deadlocks.
        let mut emit_chunk_ready = false;
        let mut emit_speech_started = false;
        let mut emit_silence_started = false;
        let mut emit_reset_to_listening = false;

        {
            let mut rec = inner_arc.blocking_lock();
            if !rec.is_recording || !rec.vad_enabled {
                // Stopping: keep the tail so stop_recording_raw sees it.
                rec.buffer.extend_from_slice(&chunk);
                drain_remaining_chunks(&events, &mut rec.buffer);
                break;
            }

            debug_frame_count += 1;
            if OPEN_MIC_DEBUG && debug_frame_count == 1 {
                log::debug!(
                    "[OpenMic] First frame: samples={} peak={:.4}",
                    chunk.len(),
                    sample::peak(&chunk)
                );
            }

            // Accumulate audio into the session buffer.
            rec.buffer.extend_from_slice(&chunk);

            // Normalize audio for VAD: some sources capture at very low levels
            // (WSLg RDPSource ~-38dB, low mic gains elsewhere). Whisper
            // normalizes internally, but Silero VAD expects normal amplitude.
            // Scale to a 0.5 target peak; frames at the noise floor stay as-is.
            // The periodic idle reset below keeps the LSTM from drifting on
            // amplified noise.
            let peak = sample::peak(&chunk);
            let vad_samples = if peak > VAD_NORMALIZE_MIN_PEAK {
                let gain = (0.5 / peak).min(100.0);
                chunk
                    .iter()
                    .map(|s| (s * gain).clamp(-1.0, 1.0))
                    .collect::<Vec<f32>>()
            } else {
                chunk.clone()
            };

            // Run VAD prediction on the normalized audio.
            let probability = vad.predict(&vad_samples);
            let is_speech = probability >= rec.vad_threshold;

            // Log every ~1s (62 frames at 16ms each) + all speech detections.
            if OPEN_MIC_DEBUG && (debug_frame_count.is_multiple_of(62) || is_speech) {
                log::debug!(
                    "[OpenMic] frame={} prob={:.3} rms={:.5} peak={:.4} speech={} state={:?}",
                    debug_frame_count,
                    probability,
                    sample::rms(&chunk),
                    peak,
                    is_speech,
                    rec.speech_state
                );
            }

            // State machine transitions (identical logic to the Pulse-era loop).
            match rec.speech_state.clone() {
                SpeechState::WaitingForSpeech => {
                    if is_speech {
                        idle_silence_frames = 0;
                        rec.speech_state = SpeechState::Speaking;
                        rec.speech_start = Some(Instant::now());
                        // Mark where speech audio begins (exclude leading silence).
                        rec.speech_start_offset = rec.buffer.len().saturating_sub(chunk.len());
                        emit_speech_started = true;
                    } else {
                        // Periodic VAD reset during long idle: keeps the Silero
                        // LSTM state from drifting due to amplified noise.
                        idle_silence_frames += 1;
                        if idle_silence_frames >= VAD_IDLE_RESET_INTERVAL {
                            vad.reset();
                            idle_silence_frames = 0;
                        }
                        // Prevent unbounded buffer growth during idle silence.
                        // Keep only ~192ms of audio as a pre-speech cushion.
                        if rec.buffer.len() > PRE_SPEECH_SAMPLES * 2 {
                            let drain_to = rec.buffer.len() - PRE_SPEECH_SAMPLES;
                            rec.buffer.drain(..drain_to);
                        }
                    }
                }
                SpeechState::Speaking => {
                    if !is_speech {
                        // Any non-speech frame starts the silence countdown.
                        // Duration check happens when silence timeout fires,
                        // NOT here - natural speech has non-speech frames
                        // (plosives, brief pauses) that would cause false resets.
                        rec.speech_state = SpeechState::SilenceAfterSpeech;
                        rec.silence_start = Some(Instant::now());
                    }
                }
                SpeechState::SilenceAfterSpeech => {
                    if is_speech {
                        // Speech resumed - go back to Speaking and notify the
                        // frontend so it can clear the silence countdown UI.
                        // Only emit if we'd previously notified about silence,
                        // to avoid spamming events on word-gap micro-pauses.
                        if rec.silence_notified {
                            emit_speech_started = true;
                        }
                        idle_silence_frames = 0;
                        rec.speech_state = SpeechState::Speaking;
                        rec.silence_start = None;
                        rec.silence_notified = false;
                        // Deliberately do NOT touch rec.speech_start here: it
                        // marks the onset of the whole segment (set once, at
                        // WaitingForSpeech -> Speaking) and must keep accumulating
                        // across SilenceAfterSpeech -> Speaking bounces so the
                        // false-trigger check below sees the segment's total
                        // speech duration, not just the time since this resume.
                    } else if let Some(silence_start) = rec.silence_start {
                        let elapsed = silence_start.elapsed().as_millis() as u64;
                        // Halfway through the silence timeout, decide: show the
                        // countdown UI, or was this a false VAD trigger (noise
                        // spike) with too little real speech? Segments shorter
                        // than MIN_SPEECH_DURATION_MS are discarded silently and
                        // the UI snaps back to Listening.
                        if elapsed >= rec.silence_timeout_ms / 2 && !rec.silence_notified {
                            // Duration of the WHOLE segment since first speech
                            // onset, measured up to silence_start so the
                            // ongoing wait is excluded. See `is_false_trigger`.
                            if is_false_trigger(rec.speech_start, silence_start) {
                                if OPEN_MIC_DEBUG {
                                    let segment_duration_ms =
                                        segment_duration_ms(rec.speech_start, silence_start);
                                    log::debug!(
                                        "[OpenMic] False trigger discarded: segment_duration={segment_duration_ms}ms < {MIN_SPEECH_DURATION_MS}ms"
                                    );
                                }
                                rec.buffer.clear();
                                rec.speech_start_offset = 0;
                                rec.speech_state = SpeechState::WaitingForSpeech;
                                rec.silence_start = None;
                                rec.silence_notified = false;
                                rec.speech_start = None;
                                vad.reset();
                                emit_reset_to_listening = true;
                            } else {
                                emit_silence_started = true;
                                rec.silence_notified = true;
                            }
                        }
                        if elapsed >= rec.silence_timeout_ms {
                            // Silence timeout - emit if the speech was long enough.
                            let speech_elapsed_ms = rec
                                .speech_start
                                .map(|t| t.elapsed().as_millis() as u64)
                                .unwrap_or(0);
                            if OPEN_MIC_DEBUG {
                                let total = rec.buffer.len();
                                let offset = rec.speech_start_offset;
                                let chunk_samples = total.saturating_sub(offset);
                                let chunk_ms = chunk_samples * 1000 / 16000;
                                log::debug!(
                                    "[OpenMic] Chunk emit: speech_elapsed={speech_elapsed_ms}ms buffer_total={total} offset={offset} chunk={chunk_samples} samples ({chunk_ms}ms)"
                                );
                            }
                            if speech_elapsed_ms >= MIN_SPEECH_DURATION_MS {
                                // Valid speech - extract only the speech audio.
                                let speech_audio = rec.buffer[rec.speech_start_offset..].to_vec();
                                // Cap the pending queue: if the frontend stalls,
                                // drop the oldest segment instead of growing
                                // unbounded. Logged so a stuck pipeline is visible.
                                while rec.pending_queue.len() >= PENDING_QUEUE_MAX {
                                    rec.pending_queue.pop_front();
                                    log::warn!(
                                        "[OpenMic] pending_queue full ({PENDING_QUEUE_MAX} items) - dropped oldest segment"
                                    );
                                }
                                rec.pending_queue.push_back(speech_audio);
                                emit_chunk_ready = true;
                            }
                            // Reset for the next utterance (emitted or discarded).
                            rec.buffer.clear();
                            rec.speech_start_offset = 0;
                            rec.speech_state = SpeechState::WaitingForSpeech;
                            rec.silence_start = None;
                            rec.silence_notified = false;
                            rec.speech_start = None;
                            vad.reset();
                        }
                    }
                }
            }
        } // lock released

        // Emit events outside the lock to avoid deadlocks.
        if emit_speech_started {
            let _ = app.emit("open-mic-speech-started", ());
        }
        if emit_silence_started {
            let _ = app.emit("open-mic-silence-started", ());
        }
        if emit_reset_to_listening {
            let _ = app.emit("open-mic-reset-to-listening", ());
        }
        if emit_chunk_ready {
            let _ = app.emit("open-mic-chunk-ready", ());
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Sentence with mid-speech bounces (Speaking -> SilenceAfterSpeech ->
    /// Speaking, repeated) is NOT a false trigger, because `speech_start` is
    /// never reset by the bounces and the whole segment cleared 500ms.
    #[test]
    fn sentence_with_mid_speech_bounces_is_not_a_false_trigger() {
        let onset = Instant::now();
        // Simulate 3 SilenceAfterSpeech->Speaking bounces at ~150ms, ~300ms,
        // ~450ms into the segment (matching the ~600ms continuous-speech
        // evidence from the debug log): speech_start stays fixed at onset
        // across every bounce (mirrors the fixed code path - it is never
        // reassigned on resume).
        let silence_start = onset + Duration::from_millis(650);

        assert!(!is_false_trigger(Some(onset), silence_start));
        assert_eq!(segment_duration_ms(Some(onset), silence_start), 650);
    }

    /// A real ~9ms/~19ms VAD blip (from the debug log) that never accumulates
    /// real speech time is still discarded.
    #[test]
    fn brief_blip_is_still_discarded() {
        let onset = Instant::now();
        let silence_start_9ms = onset + Duration::from_millis(9);
        let silence_start_19ms = onset + Duration::from_millis(19);

        assert!(is_false_trigger(Some(onset), silence_start_9ms));
        assert!(is_false_trigger(Some(onset), silence_start_19ms));
    }

    /// A segment with no recorded onset (defensive: should never happen in
    /// practice) is treated as a false trigger, not a panic or false negative.
    #[test]
    fn missing_speech_start_is_treated_as_false_trigger() {
        let now = Instant::now();
        assert!(is_false_trigger(None, now));
        assert_eq!(segment_duration_ms(None, now), 0);
    }

    /// Exactly at the threshold is NOT discarded (boundary is inclusive via
    /// `<`, matching the original real-emission check's `>=`).
    #[test]
    fn exactly_at_threshold_is_not_discarded() {
        let onset = Instant::now();
        let silence_start = onset + Duration::from_millis(MIN_SPEECH_DURATION_MS);
        assert!(!is_false_trigger(Some(onset), silence_start));
    }

    /// One ms under the threshold is discarded.
    #[test]
    fn one_ms_under_threshold_is_discarded() {
        let onset = Instant::now();
        let silence_start = onset + Duration::from_millis(MIN_SPEECH_DURATION_MS - 1);
        assert!(is_false_trigger(Some(onset), silence_start));
    }

    /// Manual stop mid-sentence (state = Speaking): collect_session_samples
    /// must return the full in-progress segment, not the ~192ms idle cushion.
    #[test]
    fn collect_session_samples_returns_full_segment_when_stopped_while_speaking() {
        let mut inner = RecorderInner::new();
        inner.buffer = vec![0.0_f32; 20_000]; // far more than the idle cushion
        inner.speech_start_offset = 1_000; // leading silence trimmed
        inner.speech_state = SpeechState::Speaking;

        let samples = collect_session_samples(&mut inner);

        assert_eq!(samples.len(), 20_000 - 1_000);
    }

    /// Manual stop during the post-speech silence countdown (state =
    /// SilenceAfterSpeech, i.e. mid-bounce or waiting out the timeout): same
    /// guarantee - the full segment survives, not just the cushion.
    #[test]
    fn collect_session_samples_returns_full_segment_when_stopped_during_silence_after_speech() {
        let mut inner = RecorderInner::new();
        inner.buffer = vec![0.0_f32; 15_000];
        inner.speech_start_offset = 2_000;
        inner.speech_state = SpeechState::SilenceAfterSpeech;

        let samples = collect_session_samples(&mut inner);

        assert_eq!(samples.len(), 15_000 - 2_000);
    }

    /// Sanity check on the WaitingForSpeech branch: with nothing pending and
    /// no in-progress segment, the whole buffer (idle cushion only) is taken.
    #[test]
    fn collect_session_samples_takes_whole_buffer_when_idle() {
        let mut inner = RecorderInner::new();
        inner.buffer = vec![0.0_f32; 3_000];
        inner.speech_state = SpeechState::WaitingForSpeech;

        let samples = collect_session_samples(&mut inner);

        assert_eq!(samples.len(), 3_000);
        assert!(inner.buffer.is_empty());
    }
}
