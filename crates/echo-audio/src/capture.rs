// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Live microphone capture via cpal (feature `capture`).
//!
//! [`start_capture`] opens the requested (or default) input device at its
//! native rate/format, downmixes each callback to mono, streams through a
//! [`StreamingResampler`] to 16 kHz, and delivers fixed-size chunks over an
//! mpsc channel:
//!
//! ```text
//! device (44.1/48 kHz, N ch) -> downmix -> resample -> chunk -> CaptureEvent
//! ```
//!
//! Consumers pick the chunk size for their use case: 256 samples for Silero
//! VAD, larger (e.g. 1024) for plain recording. All chunks are exactly
//! `chunk_samples` long except one possible shorter tail flushed on [`stop`].
//!
//! `cpal::Stream` is not `Send` on every host, so the stream is created and
//! parked on a dedicated capture thread that idles until the stop flag flips.
//!
//! **WSL exception:** cpal's Linux host is ALSA-only, but WSLg exposes the mic
//! as a PulseAudio source. When [`wsl::wslg_pulse_available`] is true the
//! capture thread spawns `parec` (16 kHz mono f32) and feeds its stdout into
//! the same [`Chunker`], reusing all downstream plumbing. cpal remains the path
//! for macOS, Windows, and non-WSL Linux - those code paths are untouched.
//!
//! [`stop`]: CaptureHandle::stop

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::resample::StreamingResampler;
use crate::{AudioError, Result, TARGET_SAMPLE_RATE};

/// How a capture session should be opened.
pub struct CaptureConfig {
    /// Input device display name; `None` uses the system default input.
    pub device_name: Option<String>,
    /// Size of each emitted chunk in 16 kHz mono samples (256 for Silero VAD).
    pub chunk_samples: usize,
}

/// What flows out of a capture session.
pub enum CaptureEvent {
    /// `chunk_samples` of 16 kHz mono audio (the final chunk may be shorter).
    Chunk(Vec<f32>),
    /// The audio stream errored; the session is dead and should be stopped.
    Error(String),
}

/// Running capture session. Dropping without [`CaptureHandle::stop`] also
/// stops the stream (via the thread's stop-flag check), but `stop` joins the
/// thread and guarantees the tail chunk has been flushed.
pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    source_rate: u32,
}

impl CaptureHandle {
    /// Signal the capture thread to stop, flush the tail chunk, and join.
    pub fn stop(mut self) {
        self.stop_and_join();
    }

    /// The device's native sample rate (before resampling to 16 kHz).
    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    fn stop_and_join(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

/// Start capturing the microphone. Returns the session handle and the event
/// receiver. Fails fast (before returning) if the device cannot be opened.
pub fn start_capture(config: CaptureConfig) -> Result<(CaptureHandle, Receiver<CaptureEvent>)> {
    let (event_tx, event_rx) = mpsc::channel::<CaptureEvent>();
    let (setup_tx, setup_rx) = mpsc::channel::<Result<u32>>();
    let stop = Arc::new(AtomicBool::new(false));

    let thread_stop = stop.clone();
    let thread = std::thread::Builder::new()
        .name("echo-audio-capture".into())
        .spawn(move || run_capture_thread(config, event_tx, setup_tx, thread_stop))
        .map_err(|e| AudioError::Capture(format!("spawning capture thread: {e}")))?;

    // Wait for the thread to report whether the device opened.
    let source_rate = setup_rx
        .recv()
        .map_err(|_| AudioError::Capture("capture thread died during setup".into()))??;

    Ok((
        CaptureHandle {
            stop,
            thread: Some(thread),
            source_rate,
        },
        event_rx,
    ))
}

/// Body of the capture thread: open the device, run the stream until the stop
/// flag flips, then flush the tail chunk.
fn run_capture_thread(
    config: CaptureConfig,
    events: Sender<CaptureEvent>,
    setup: Sender<Result<u32>>,
    stop: Arc<AtomicBool>,
) {
    // Shared between the audio callback (feed) and this thread (tail flush).
    let chunker = Arc::new(Mutex::new(Chunker::new(
        config.chunk_samples,
        events.clone(),
    )));

    // WSLg routes the mic through PulseAudio, which cpal's ALSA-only Linux host
    // cannot open. Capture via `parec` instead; everything downstream is shared.
    #[cfg(target_os = "linux")]
    if crate::wsl::wslg_pulse_available() {
        run_parec_capture(&config, chunker, events, setup, stop);
        return;
    }

    let stream = match open_stream(&config, chunker.clone(), events.clone()) {
        Ok((stream, source_rate)) => {
            // Tell the resampler its real input rate now that we know it.
            chunker.lock().unwrap().set_source_rate(source_rate);
            let _ = setup.send(Ok(source_rate));
            stream
        }
        Err(e) => {
            let _ = setup.send(Err(e));
            return;
        }
    };

    if let Err(e) = stream.play() {
        let _ = events.send(CaptureEvent::Error(format!("starting audio stream: {e}")));
        return;
    }

    // Idle until asked to stop; the audio callback does all the work.
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }

    drop(stream); // no more callbacks after this
    chunker.lock().unwrap().flush_tail();
}

/// WSL capture path: stream the mic from `parec` (PulseAudio) into the shared
/// [`Chunker`]. Requested at 16 kHz mono f32, so the resampler is a passthrough
/// and no format conversion is needed. Honors `config.device_name` as a Pulse
/// source name; `None` lets `parec` pick the default source (`RDPSource` on
/// WSLg), so both "System Default" and an explicit RDP selection work.
#[cfg(target_os = "linux")]
fn run_parec_capture(
    config: &CaptureConfig,
    chunker: Arc<Mutex<Chunker>>,
    events: Sender<CaptureEvent>,
    setup: Sender<Result<u32>>,
    stop: Arc<AtomicBool>,
) {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("parec");
    cmd.arg("--format=float32le")
        .arg("--rate=16000")
        .arg("--channels=1")
        // main() exports PULSE_SERVER already; set it defensively so a bare
        // capture (tests, non-Tauri callers) still reaches the WSLg bridge.
        .env("PULSE_SERVER", crate::wsl::WSLG_PULSE_SERVER)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(device) = config.device_name.as_deref() {
        cmd.arg(format!("--device={device}"));
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = setup.send(Err(AudioError::Capture(format!(
                "parec unavailable (install pulseaudio-utils): {e}"
            ))));
            return;
        }
    };

    let mut stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = setup.send(Err(AudioError::Capture("parec produced no stdout".into())));
            let _ = child.kill();
            return;
        }
    };

    // 16 kHz in == 16 kHz out: passthrough resampler. Report rate now so
    // start_capture's fail-fast handshake unblocks with success.
    chunker.lock().unwrap().set_source_rate(TARGET_SAMPLE_RATE);
    let _ = setup.send(Ok(TARGET_SAMPLE_RATE));

    let mut read_buf = [0u8; 4096];
    let mut carry: Vec<u8> = Vec::new();
    let mut samples: Vec<f32> = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        match stdout.read(&mut read_buf) {
            Ok(0) => break, // EOF: parec exited on its own.
            Ok(n) => {
                samples.clear();
                drain_f32le(&mut carry, &read_buf[..n], &mut samples);
                if !samples.is_empty()
                    && let Ok(mut c) = chunker.lock()
                {
                    c.feed(&samples);
                }
            }
            Err(e) => {
                let _ = events.send(CaptureEvent::Error(format!("parec read error: {e}")));
                break;
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    chunker.lock().unwrap().flush_tail();
}

/// Reinterpret a little-endian f32 byte stream as samples, appending to `out`.
/// Any trailing bytes that don't complete a 4-byte sample are held in `carry`
/// and prepended on the next call, so reads that split a sample never corrupt
/// or drop it.
#[cfg(target_os = "linux")]
fn drain_f32le(carry: &mut Vec<u8>, incoming: &[u8], out: &mut Vec<f32>) {
    carry.extend_from_slice(incoming);
    let full = carry.len() - (carry.len() % 4);
    for frame in carry[..full].as_chunks::<4>().0 {
        out.push(f32::from_le_bytes(*frame));
    }
    carry.drain(..full);
}

/// Open the configured input device and build a stream for its sample format.
/// Returns the stream and the device's native sample rate.
fn open_stream(
    config: &CaptureConfig,
    chunker: Arc<Mutex<Chunker>>,
    events: Sender<CaptureEvent>,
) -> Result<(cpal::Stream, u32)> {
    let device = open_input_device(config.device_name.as_deref())?;
    let supported = device
        .default_input_config()
        .map_err(|e| AudioError::Capture(format!("querying default input config: {e}")))?;
    let sample_format = supported.sample_format();
    let stream_config: cpal::StreamConfig = supported.into();
    let channels = stream_config.channels;
    let source_rate = stream_config.sample_rate;

    let err_events = events;
    let err_fn = move |e: cpal::Error| {
        if is_recoverable_stream_error(e.kind()) {
            // The stream keeps running; killing the recording over a glitch
            // made stop fail with "Not recording" (cpal 0.18 reports Xrun).
            eprintln!("[echo-audio] recoverable audio stream error (ignored): {e}");
            return;
        }
        let _ = err_events.send(CaptureEvent::Error(format!("audio stream error: {e}")));
    };

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_stream_for_format::<f32>(
            &device,
            &stream_config,
            channels,
            chunker,
            err_fn,
            |s| s,
        )?,
        cpal::SampleFormat::I16 => build_stream_for_format::<i16>(
            &device,
            &stream_config,
            channels,
            chunker,
            err_fn,
            |s| s as f32 / 32768.0,
        )?,
        cpal::SampleFormat::U16 => build_stream_for_format::<u16>(
            &device,
            &stream_config,
            channels,
            chunker,
            err_fn,
            |s| (s as f32 - 32768.0) / 32768.0,
        )?,
        other => {
            return Err(AudioError::Capture(format!(
                "unsupported sample format: {other:?}"
            )));
        }
    };

    Ok((stream, source_rate))
}

/// Find the named input device, or the system default when `name` is `None`.
fn open_input_device(name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    match name {
        Some(name) => host
            .input_devices()
            .map_err(|e| AudioError::Capture(format!("enumerating input devices: {e}")))?
            .find(|d| d.description().map(|d| d.name() == name).unwrap_or(false))
            .ok_or_else(|| AudioError::Capture(format!("input device '{name}' not found"))),
        None => host
            .default_input_device()
            .ok_or_else(|| AudioError::Capture("no default input device available".into())),
    }
}

/// Build an input stream for sample type `T`: each callback downmixes the
/// interleaved frames to mono f32 (via `convert`) and feeds the chunker.
/// Stream errors after which cpal keeps the stream alive: a buffer glitch,
/// an automatic reroute, or a refused realtime promotion. Everything else
/// means the session is dead.
fn is_recoverable_stream_error(kind: cpal::ErrorKind) -> bool {
    matches!(
        kind,
        cpal::ErrorKind::Xrun | cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied
    )
}

fn build_stream_for_format<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: u16,
    chunker: Arc<Mutex<Chunker>>,
    err_fn: impl FnMut(cpal::Error) + Send + 'static,
    convert: impl Fn(T) -> f32 + Send + 'static,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + Send + 'static,
{
    let frame = channels.max(1) as usize;
    let mut mono_scratch: Vec<f32> = Vec::new();
    let stream = device
        .build_input_stream::<T, _, _>(
            *config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Downmix interleaved frames into the reusable scratch buffer.
                mono_scratch.clear();
                for chunk in data.chunks(frame) {
                    let mut acc = 0.0f32;
                    for &s in chunk {
                        acc += convert(s);
                    }
                    mono_scratch.push(acc / chunk.len() as f32);
                }
                if let Ok(mut c) = chunker.lock() {
                    c.feed(&mono_scratch);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| AudioError::Capture(format!("building input stream: {e}")))?;
    Ok(stream)
}

/// Resamples incoming mono audio to 16 kHz and emits fixed-size chunks.
struct Chunker {
    resampler: StreamingResampler,
    pending: Vec<f32>,
    chunk_samples: usize,
    events: Sender<CaptureEvent>,
}

impl Chunker {
    fn new(chunk_samples: usize, events: Sender<CaptureEvent>) -> Self {
        Self {
            // Placeholder rate; corrected via set_source_rate before playback.
            resampler: StreamingResampler::new(TARGET_SAMPLE_RATE, TARGET_SAMPLE_RATE),
            pending: Vec::new(),
            chunk_samples: chunk_samples.max(1),
            events,
        }
    }

    /// Set the device's real input rate (known only after the device opens).
    fn set_source_rate(&mut self, source_rate: u32) {
        self.resampler = StreamingResampler::new(source_rate, TARGET_SAMPLE_RATE);
    }

    /// Ingest mono samples at the device rate; emit every full chunk.
    fn feed(&mut self, mono: &[f32]) {
        self.resampler.process(mono, &mut self.pending);
        while self.pending.len() >= self.chunk_samples {
            let chunk: Vec<f32> = self.pending.drain(..self.chunk_samples).collect();
            let _ = self.events.send(CaptureEvent::Chunk(chunk));
        }
    }

    /// Emit whatever remains as a final (shorter) chunk. Called after the
    /// stream is dropped, so no callback can race this.
    fn flush_tail(&mut self) {
        if !self.pending.is_empty() {
            let tail = std::mem::take(&mut self.pending);
            let _ = self.events.send(CaptureEvent::Chunk(tail));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chunker unit test: no device needed - drive it directly.
    #[test]
    fn chunker_emits_fixed_chunks_and_tail() {
        let (tx, rx) = mpsc::channel();
        let mut c = Chunker::new(4, tx);
        c.set_source_rate(TARGET_SAMPLE_RATE); // passthrough

        c.feed(&[0.1; 6]); // 6 in -> one chunk of 4, 2 pending
        c.feed(&[0.2; 3]); // 5 pending -> one chunk of 4, 1 pending
        c.flush_tail(); // tail of 1

        let sizes: Vec<usize> = rx
            .try_iter()
            .map(|e| match e {
                CaptureEvent::Chunk(c) => c.len(),
                CaptureEvent::Error(e) => panic!("unexpected error: {e}"),
            })
            .collect();
        assert_eq!(sizes, vec![4, 4, 1]);
    }

    /// The WSL `parec` reader hands raw little-endian f32 bytes to `drain_f32le`,
    /// which must reassemble the exact samples and route them through the same
    /// Chunker used by cpal - even when a read splits a sample across buffers.
    #[cfg(target_os = "linux")]
    #[test]
    fn drain_f32le_reassembles_split_samples_and_feeds_chunker() {
        let source = [0.1f32, -0.2, 0.3, 0.4, -0.5, 0.6];
        let mut bytes: Vec<u8> = Vec::new();
        for s in source {
            bytes.extend_from_slice(&s.to_le_bytes());
        }

        // Feed the byte stream in awkward 5-byte reads so most reads land
        // mid-sample, exercising the carry buffer.
        let mut carry: Vec<u8> = Vec::new();
        let mut decoded: Vec<f32> = Vec::new();
        for piece in bytes.chunks(5) {
            drain_f32le(&mut carry, piece, &mut decoded);
        }

        assert!(carry.is_empty(), "no partial bytes should remain");
        assert_eq!(decoded.len(), source.len());
        for (got, want) in decoded.iter().zip(source.iter()) {
            assert!((got - want).abs() < f32::EPSILON, "got {got}, want {want}");
        }

        // Now confirm the decoded samples emit correct fixed chunks + tail.
        let (tx, rx) = mpsc::channel();
        let mut c = Chunker::new(4, tx);
        c.set_source_rate(TARGET_SAMPLE_RATE); // passthrough
        c.feed(&decoded); // 6 samples -> one chunk of 4, 2 pending
        c.flush_tail(); // tail of 2

        let sizes: Vec<usize> = rx
            .try_iter()
            .map(|e| match e {
                CaptureEvent::Chunk(c) => c.len(),
                CaptureEvent::Error(e) => panic!("unexpected error: {e}"),
            })
            .collect();
        assert_eq!(sizes, vec![4, 2]);
    }

    #[test]
    fn chunker_resamples_48k_to_16k() {
        let (tx, rx) = mpsc::channel();
        let mut c = Chunker::new(100, tx);
        c.set_source_rate(48_000);

        c.feed(&vec![0.5; 1200]); // 1200 @ 48k -> ~400 @ 16k -> 4 chunks
        c.flush_tail();

        let total: usize = rx
            .try_iter()
            .map(|e| match e {
                CaptureEvent::Chunk(c) => c.len(),
                CaptureEvent::Error(e) => panic!("unexpected error: {e}"),
            })
            .sum();
        assert!((399..=400).contains(&total), "total {total}");
    }

    #[test]
    fn glitch_errors_do_not_kill_the_session() {
        assert!(is_recoverable_stream_error(cpal::ErrorKind::Xrun));
        assert!(is_recoverable_stream_error(cpal::ErrorKind::DeviceChanged));
        assert!(is_recoverable_stream_error(cpal::ErrorKind::RealtimeDenied));
        assert!(!is_recoverable_stream_error(
            cpal::ErrorKind::DeviceNotAvailable
        ));
        assert!(!is_recoverable_stream_error(
            cpal::ErrorKind::StreamInvalidated
        ));
        assert!(!is_recoverable_stream_error(cpal::ErrorKind::BackendError));
    }
}
