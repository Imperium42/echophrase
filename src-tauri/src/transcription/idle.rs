// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Free the speech model's memory after a stretch of disuse (opt-in).
//!
//! The loaded model is cached for the life of the process so dictation starts
//! instantly. That is the right trade while the user is dictating, and the
//! wrong one when Echophrase sits hidden in the tray for hours: the weights
//! (and whatever ORT's allocators kept from the biggest clip ever run) stay
//! resident and compete with a game for GPU and system memory.
//!
//! The behaviour is OFF by default because the next recording pays a model
//! reload (about 7 s on a warm shader cache, longer cold). When the user
//! enables it in Settings > Optimization, a background watcher calls
//! `unload_model` after the chosen number of idle minutes. The next recording
//! start notices the unload and reloads the model in the background while the
//! user is still speaking, so the cost is normally hidden behind the recording.
//!
//! The decision logic lives in [`IdleTracker`], which takes `Instant`s as
//! arguments so it is deterministic under test.

use parking_lot::Mutex;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// Default idle minutes shown in Settings (the feature itself defaults to off).
pub const DEFAULT_IDLE_UNLOAD_MINUTES: u32 = 10;
/// Lowest idle timeout the setting accepts.
pub const MIN_IDLE_UNLOAD_MINUTES: u32 = 1;
/// Highest idle timeout the setting accepts (2 hours).
pub const MAX_IDLE_UNLOAD_MINUTES: u32 = 120;

/// Idle time before unload for the given settings, or `None` when the feature
/// is disabled. Out-of-range minutes are clamped rather than rejected so a
/// hand-edited settings file cannot disable the guard by accident.
pub fn idle_threshold(enabled: bool, minutes: u32) -> Option<Duration> {
    enabled.then(|| {
        let minutes = minutes.clamp(MIN_IDLE_UNLOAD_MINUTES, MAX_IDLE_UNLOAD_MINUTES);
        Duration::from_secs(u64::from(minutes) * 60)
    })
}

/// How often the watcher thread checks whether the model is due for unload.
const CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Pure idle-state machine. Time is always passed in.
#[derive(Debug)]
struct IdleTracker {
    /// Last time anything used (or finished using) the model.
    last_activity: Instant,
    /// Transcriptions currently running. The model is never unloaded while > 0.
    in_flight: usize,
    /// True once the watcher has unloaded the model and nothing has used it since.
    unloaded: bool,
}

impl IdleTracker {
    /// Start tracking from `now` (app launch counts as activity).
    fn new(now: Instant) -> Self {
        Self {
            last_activity: now,
            in_flight: 0,
            unloaded: false,
        }
    }

    /// Record activity at `now`. Returns whether the model had been unloaded
    /// for idleness, so the caller can warm it back up.
    fn touch(&mut self, now: Instant) -> bool {
        self.last_activity = now;
        std::mem::replace(&mut self.unloaded, false)
    }

    /// Restart the idle countdown without touching the unloaded flag. Used
    /// while the feature is disabled so enabling it starts a fresh countdown.
    fn restart_clock(&mut self, now: Instant) {
        self.last_activity = now;
    }

    /// A transcription starts: counts as activity and blocks unloading.
    fn begin(&mut self, now: Instant) {
        self.touch(now);
        self.in_flight += 1;
    }

    /// A transcription ends. Idle time is measured from this moment.
    fn end(&mut self, now: Instant) {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.last_activity = now;
    }

    /// Whether the model should be unloaded at `now`.
    fn due_for_unload(&self, now: Instant, threshold: Duration) -> bool {
        self.in_flight == 0
            && !self.unloaded
            && now.saturating_duration_since(self.last_activity) >= threshold
    }

    /// Note that the watcher unloaded the model.
    fn mark_unloaded(&mut self) {
        self.unloaded = true;
    }
}

/// Process-wide tracker shared by the transcription and recording paths.
static TRACKER: LazyLock<Mutex<IdleTracker>> =
    LazyLock::new(|| Mutex::new(IdleTracker::new(Instant::now())));

/// RAII marker for a running transcription: blocks idle unload while alive and
/// restarts the idle clock when dropped, on every exit path.
pub struct ActivityGuard {
    _private: (),
}

/// Mark a transcription as running. Hold the guard for its whole duration.
pub fn begin() -> ActivityGuard {
    TRACKER.lock().begin(Instant::now());
    ActivityGuard { _private: () }
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        TRACKER.lock().end(Instant::now());
    }
}

/// Call when a recording starts. Returns `true` when the model was unloaded for
/// idleness and the caller should reload it in the background.
pub fn on_recording_start() -> bool {
    TRACKER.lock().touch(Instant::now())
}

/// Start the background watcher that unloads the model after idle time.
pub fn spawn_watcher(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("model-idle-unload".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(CHECK_INTERVAL);
                watch_tick(&app, Instant::now());
            }
        });
    if let Err(e) = spawned {
        log::warn!("[idle] could not start model idle watcher: {e}");
    }
}

/// One watcher pass: unload the model if it has been idle long enough.
fn watch_tick(app: &AppHandle, now: Instant) {
    // Read the setting every pass so toggling it takes effect without a restart.
    let threshold = crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| idle_threshold(s.idle_unload_enabled, s.idle_unload_minutes));

    let mut tracker = TRACKER.lock();

    // A recording (including a long open-mic session) or transcription is in
    // progress: the app is not idle, whatever the clock says.
    if crate::tray_status::is_busy(app) {
        tracker.touch(now);
        return;
    }

    // Disabled: keep the countdown fresh so turning it on starts from now.
    let Some(threshold) = threshold else {
        tracker.restart_clock(now);
        return;
    };

    // The tracker lock is held across the unload so a transcription cannot
    // begin between the decision and the cache being cleared.
    if tracker.due_for_unload(now, threshold) {
        log::info!(
            "[idle] no activity for {}s - unloading the model to free memory",
            threshold.as_secs()
        );
        crate::commands::transcription::unload_model();
        tracker.mark_unloaded();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THRESHOLD: Duration = Duration::from_secs(600);

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn not_due_before_the_threshold() {
        let t0 = Instant::now();
        let tracker = IdleTracker::new(t0);
        assert!(!tracker.due_for_unload(t0 + secs(599), THRESHOLD));
    }

    #[test]
    fn due_at_and_after_the_threshold() {
        let t0 = Instant::now();
        let tracker = IdleTracker::new(t0);
        assert!(tracker.due_for_unload(t0 + secs(600), THRESHOLD));
        assert!(tracker.due_for_unload(t0 + secs(3600), THRESHOLD));
    }

    #[test]
    fn never_due_while_a_transcription_is_running() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.begin(t0);
        assert!(!tracker.due_for_unload(t0 + secs(7200), THRESHOLD));
    }

    #[test]
    fn idle_clock_restarts_when_a_transcription_ends() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.begin(t0);
        tracker.end(t0 + secs(300));
        // 300 s after the END, not 600 s after the start.
        assert!(!tracker.due_for_unload(t0 + secs(300 + 599), THRESHOLD));
        assert!(tracker.due_for_unload(t0 + secs(300 + 600), THRESHOLD));
    }

    #[test]
    fn overlapping_transcriptions_block_until_both_finish() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.begin(t0);
        tracker.begin(t0);
        tracker.end(t0 + secs(10));
        assert!(!tracker.due_for_unload(t0 + secs(5000), THRESHOLD));
        tracker.end(t0 + secs(20));
        assert!(tracker.due_for_unload(t0 + secs(20 + 600), THRESHOLD));
    }

    #[test]
    fn unbalanced_end_does_not_underflow() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.end(t0 + secs(1));
        tracker.begin(t0 + secs(2));
        assert!(!tracker.due_for_unload(t0 + secs(5000), THRESHOLD));
    }

    #[test]
    fn unloads_only_once_per_idle_period() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        assert!(tracker.due_for_unload(t0 + secs(600), THRESHOLD));
        tracker.mark_unloaded();
        assert!(!tracker.due_for_unload(t0 + secs(1200), THRESHOLD));
    }

    #[test]
    fn due_again_after_new_activity_and_another_idle_period() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.mark_unloaded();
        tracker.touch(t0 + secs(700));
        assert!(!tracker.due_for_unload(t0 + secs(700 + 599), THRESHOLD));
        assert!(tracker.due_for_unload(t0 + secs(700 + 600), THRESHOLD));
    }

    #[test]
    fn touch_reports_whether_the_model_had_been_unloaded() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        assert!(!tracker.touch(t0 + secs(1)), "nothing was unloaded yet");
        tracker.mark_unloaded();
        assert!(tracker.touch(t0 + secs(2)), "model had been unloaded");
        assert!(
            !tracker.touch(t0 + secs(3)),
            "warm-up is reported only once"
        );
    }

    #[test]
    fn threshold_is_none_when_disabled() {
        assert_eq!(idle_threshold(false, 10), None);
    }

    #[test]
    fn threshold_converts_minutes_to_a_duration() {
        assert_eq!(idle_threshold(true, 10), Some(Duration::from_secs(600)));
        assert_eq!(idle_threshold(true, 1), Some(Duration::from_secs(60)));
    }

    #[test]
    fn threshold_clamps_out_of_range_minutes() {
        assert_eq!(idle_threshold(true, 0), Some(Duration::from_secs(60)));
        assert_eq!(
            idle_threshold(true, 100_000),
            Some(Duration::from_secs(u64::from(MAX_IDLE_UNLOAD_MINUTES) * 60))
        );
    }

    #[test]
    fn restart_clock_does_not_clear_the_unloaded_flag() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.mark_unloaded();
        tracker.restart_clock(t0 + secs(5));
        assert!(
            tracker.touch(t0 + secs(6)),
            "model is still unloaded, so the next recording must warm it"
        );
    }

    #[test]
    fn restart_clock_delays_the_unload() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.restart_clock(t0 + secs(500));
        assert!(!tracker.due_for_unload(t0 + secs(500 + 599), THRESHOLD));
        assert!(tracker.due_for_unload(t0 + secs(500 + 600), THRESHOLD));
    }

    #[test]
    fn begin_also_clears_the_unloaded_flag() {
        let t0 = Instant::now();
        let mut tracker = IdleTracker::new(t0);
        tracker.mark_unloaded();
        // A transcription lazily reloads the model, so it is no longer unloaded.
        tracker.begin(t0 + secs(1));
        tracker.end(t0 + secs(2));
        assert!(!tracker.touch(t0 + secs(3)));
    }
}
