// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Split long recordings into bounded chunks before they reach the model.
//!
//! Parakeet's encoder working memory grows much faster than clip length, and
//! ORT keeps the peak for the life of the session. Measured on parakeet-rs
//! 0.3.4 / ort rc.12 (CPU EP, peak RSS above the loaded model): 10 s +0.3 GB,
//! 60 s +0.6 GB, 120 s +0.8 GB, 300 s +3.3 GB. A real Windows install reached
//! ~4.5 GB after a single accidental 300 s recording (see BENCHMARKS.md).
//!
//! Feeding the model bounded chunks caps that peak no matter how long the
//! recording is. Cuts land on the quietest stretch near each target boundary,
//! so words are not sliced in half.

use std::ops::Range;

/// Preferred chunk length. Peak working memory at this size is a small
/// fraction of the 300 s case.
pub const TARGET_CHUNK_SECS: f32 = 30.0;

/// Hard ceiling for one chunk. Audio at or below this goes to the model whole.
pub const MAX_CHUNK_SECS: f32 = 40.0;

/// How far either side of the target boundary to look for a quiet spot.
/// Must satisfy `TARGET + SEARCH <= MAX` so a chunk never exceeds the ceiling.
pub const SEARCH_WINDOW_SECS: f32 = 5.0;

/// Energy is measured over frames of this length when hunting for a quiet cut.
const FRAME_SECS: f32 = 0.05;

/// Convert seconds to a sample count at `sample_rate`.
fn secs_to_samples(secs: f32, sample_rate: u32) -> usize {
    (secs * sample_rate as f32) as usize
}

/// Plan how to split `samples` into contiguous chunks no longer than
/// [`MAX_CHUNK_SECS`].
///
/// The returned ranges are contiguous, non-overlapping and together cover
/// `0..samples.len()` exactly. Audio at or below the ceiling (including empty
/// input) comes back as a single range, so short clips behave as before.
pub fn plan_chunks(samples: &[f32], sample_rate: u32) -> Vec<Range<usize>> {
    plan_chunks_with(
        samples,
        sample_rate,
        TARGET_CHUNK_SECS,
        MAX_CHUNK_SECS,
        SEARCH_WINDOW_SECS,
    )
}

/// Like [`plan_chunks`] with explicit limits, for models with a tighter
/// per-call ceiling than Parakeet. Requires `target + window <= max`.
pub fn plan_chunks_with(
    samples: &[f32],
    sample_rate: u32,
    target_secs: f32,
    max_secs: f32,
    window_secs: f32,
) -> Vec<Range<usize>> {
    debug_assert!(target_secs + window_secs <= max_secs);
    let max_len: usize = secs_to_samples(max_secs, sample_rate);
    let target: usize = secs_to_samples(target_secs, sample_rate);
    let window: usize = secs_to_samples(window_secs, sample_rate);
    let frame: usize = secs_to_samples(FRAME_SECS, sample_rate).max(1);

    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut start: usize = 0;

    // Strictly greater: audio exactly at the ceiling goes through whole. The
    // search range stays inside `start + max_len`, which is below the end
    // while this loop runs, so every cut is a valid index.
    while samples.len() - start > max_len {
        let search_start: usize = start + target.saturating_sub(window);
        let search_end: usize = (start + target + window).min(start + max_len);
        let cut: usize = quietest_cut(samples, search_start, search_end, start + target, frame);
        ranges.push(start..cut);
        start = cut;
    }

    ranges.push(start..samples.len());
    ranges
}

/// Pick a cut index in `search_start..search_end`: the middle of the
/// lowest-energy frame, preferring the frame closest to `target` on ties
/// (steady noise or a long pause must still cut near the intended length).
fn quietest_cut(
    samples: &[f32],
    search_start: usize,
    search_end: usize,
    target: usize,
    frame: usize,
) -> usize {
    // Search range too small to hold a frame: split down the middle.
    if search_end < search_start + frame {
        return search_start + (search_end - search_start) / 2;
    }

    let mut best_cut: usize = search_start + frame / 2;
    let mut best_energy: f32 = f32::MAX;
    let mut best_distance: usize = usize::MAX;

    let mut frame_start: usize = search_start;
    while frame_start + frame <= search_end {
        let slice: &[f32] = &samples[frame_start..frame_start + frame];
        let energy: f32 = slice.iter().map(|s| s * s).sum::<f32>() / frame as f32;
        let cut: usize = frame_start + frame / 2;
        let distance: usize = cut.abs_diff(target);

        if energy < best_energy || (energy == best_energy && distance < best_distance) {
            best_energy = energy;
            best_distance = distance;
            best_cut = cut;
        }
        frame_start += frame;
    }

    best_cut
}

/// Join per-chunk transcripts into one string: trimmed, empty chunks dropped,
/// separated by a single space.
pub fn join_chunk_texts(texts: &[String]) -> String {
    texts
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 16_000;

    #[test]
    fn custom_limits_never_exceed_ceiling_and_cut_in_silence() {
        // 70 s of speech with a pause centred at 26 s (inside 20..30 search).
        let mut audio = loud(25.0);
        audio.extend(quiet(2.0));
        audio.extend(loud(43.0));
        let ranges = plan_chunks_with(&audio, SR, 25.0, 30.0, 5.0);
        assert!(ranges.len() >= 3);
        for r in &ranges {
            assert!(r.len() <= 30 * SR as usize);
        }
        let first_cut = ranges[0].end;
        assert!(first_cut >= 25 * SR as usize && first_cut <= 27 * SR as usize);
        assert_eq!(ranges.last().unwrap().end, audio.len());
    }

    /// Constant-amplitude "speech" for `secs` seconds.
    fn loud(secs: f32) -> Vec<f32> {
        vec![0.5; (secs * SR as f32) as usize]
    }

    /// Digital silence for `secs` seconds.
    fn quiet(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * SR as f32) as usize]
    }

    /// Assert ranges are contiguous and cover exactly `0..len`.
    fn assert_exact_cover(ranges: &[Range<usize>], len: usize) {
        assert!(!ranges.is_empty());
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[ranges.len() - 1].end, len);
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "gap or overlap between chunks");
        }
    }

    #[test]
    fn empty_input_is_a_single_empty_range() {
        assert_eq!(plan_chunks(&[], SR), vec![0..0]);
    }

    #[test]
    fn short_clip_is_not_split() {
        let samples = loud(10.0);
        assert_eq!(plan_chunks(&samples, SR), vec![0..samples.len()]);
    }

    #[test]
    fn clip_exactly_at_ceiling_is_not_split() {
        let samples = loud(MAX_CHUNK_SECS);
        assert_eq!(plan_chunks(&samples, SR).len(), 1);
    }

    #[test]
    fn long_clip_is_split_into_bounded_chunks_covering_everything() {
        let samples = loud(300.0);
        let ranges = plan_chunks(&samples, SR);
        assert!(ranges.len() > 1);
        assert_exact_cover(&ranges, samples.len());
        let max_len = secs_to_samples(MAX_CHUNK_SECS, SR);
        for r in &ranges {
            assert!(
                r.len() <= max_len,
                "chunk of {} samples exceeds ceiling",
                r.len()
            );
        }
    }

    #[test]
    fn no_tiny_tail_chunk() {
        // 40.5 s: just over the ceiling. The tail must not be a sliver.
        let samples = loud(40.5);
        let ranges = plan_chunks(&samples, SR);
        assert_eq!(ranges.len(), 2);
        let tail = ranges[1].len();
        assert!(
            tail >= secs_to_samples(5.0, SR),
            "tail was only {tail} samples"
        );
    }

    #[test]
    fn cuts_land_in_the_quiet_gap_near_the_target() {
        // 31 s speech, 1 s pause, 40 s speech, 1 s pause, 30 s speech.
        let mut samples = loud(31.0);
        samples.extend(quiet(1.0));
        samples.extend(loud(40.0));
        samples.extend(quiet(1.0));
        samples.extend(loud(30.0));

        let ranges = plan_chunks(&samples, SR);
        assert_exact_cover(&ranges, samples.len());

        let first_gap = secs_to_samples(31.0, SR)..secs_to_samples(32.0, SR);
        assert!(
            first_gap.contains(&ranges[0].end),
            "first cut at {} samples missed the pause {:?}",
            ranges[0].end,
            first_gap
        );
    }

    #[test]
    fn sample_rate_scales_the_chunk_size() {
        // The same 100 s of audio at 8 kHz must split the same way in time.
        let samples = vec![0.5f32; 100 * 8_000];
        let ranges = plan_chunks(&samples, 8_000);
        assert!(ranges.len() > 1);
        let max_len = secs_to_samples(MAX_CHUNK_SECS, 8_000);
        assert!(ranges.iter().all(|r| r.len() <= max_len));
    }

    #[test]
    fn join_trims_skips_empty_and_single_spaces() {
        let texts = vec![
            " Hello there. ".to_string(),
            "".to_string(),
            "   ".to_string(),
            "General Kenobi.".to_string(),
        ];
        assert_eq!(join_chunk_texts(&texts), "Hello there. General Kenobi.");
    }

    #[test]
    fn join_of_nothing_is_empty() {
        assert_eq!(join_chunk_texts(&[]), "");
    }
}
