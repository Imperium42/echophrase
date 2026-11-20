// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Prompt template construction for Qwen3-ASR.
//!
//! Standard prompt (no language hint):
//!   `<|im_start|>system\n<|im_end|>\n<|im_start|>user\n<|audio_start|><|audio_pad|>*N<|audio_end|><|im_end|>\n<|im_start|>assistant\n`
//!
//! With language hint (official Qwen3-ASR template), the assistant prefix is
//! extended with `language {Name}<asr_text>`, forcing the model to skip
//! language detection and begin transcription immediately.

use super::config::SpecialTokens;

/// Well-known text token IDs from the Qwen3-ASR (Qwen2.5-family) tokenizer:
/// "system", "user", "assistant", "\n", and "language".
const SYSTEM_TOKEN_ID: i64 = 9125;
const USER_TOKEN_ID: i64 = 882;
const ASSISTANT_TOKEN_ID: i64 = 77091;
const NEWLINE_TOKEN_ID: i64 = 198;
const LANGUAGE_TOKEN_ID: i64 = 11528;

/// Number of encoder output tokens for a mel frame count. Matches the native
/// Qwen3-ASR formula: 100-frame conv windows, three stride-2 layers on the
/// remainder, 13 tokens per full window.
pub fn get_feat_extract_output_lengths(input_lengths: usize) -> usize {
    let leave = input_lengths % 100;
    let t = leave.div_ceil(2).div_ceil(2).div_ceil(2);
    t + (input_lengths / 100) * 13
}

/// Build the complete prompt token ID sequence, with an optional forced
/// `language {Name}<asr_text>` assistant prefix. `language_token_ids` should
/// be the tokenized form of ` {Name}` (with leading space).
pub fn build_prompt_ids(
    special_tokens: &SpecialTokens,
    audio_token_count: usize,
    language_token_ids: Option<&[i64]>,
) -> Vec<i64> {
    let mut ids = Vec::with_capacity(audio_token_count + 20);

    // System turn (empty content)
    ids.push(special_tokens.im_start_token_id);
    ids.push(SYSTEM_TOKEN_ID);
    ids.push(NEWLINE_TOKEN_ID);
    ids.push(special_tokens.im_end_token_id);
    ids.push(NEWLINE_TOKEN_ID);

    // User turn with audio pad tokens (replaced by encoder embeddings at runtime)
    ids.push(special_tokens.im_start_token_id);
    ids.push(USER_TOKEN_ID);
    ids.push(NEWLINE_TOKEN_ID);
    ids.push(special_tokens.audio_start_token_id);
    ids.extend(std::iter::repeat_n(
        special_tokens.audio_pad_token_id,
        audio_token_count,
    ));
    ids.push(special_tokens.audio_end_token_id);
    ids.push(special_tokens.im_end_token_id);
    ids.push(NEWLINE_TOKEN_ID);

    // Assistant turn prefix
    ids.push(special_tokens.im_start_token_id);
    ids.push(ASSISTANT_TOKEN_ID);
    ids.push(NEWLINE_TOKEN_ID);

    if let Some(lang_ids) = language_token_ids {
        ids.push(LANGUAGE_TOKEN_ID);
        ids.extend_from_slice(lang_ids);
        ids.push(special_tokens.asr_text_token_id);
    }

    ids
}

/// Find the first `audio_pad` token position in the prompt.
pub fn get_audio_pad_start(prompt_ids: &[i64], audio_pad_token_id: i64) -> Option<usize> {
    prompt_ids.iter().position(|&id| id == audio_pad_token_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_special_tokens() -> SpecialTokens {
        SpecialTokens {
            eos_token_ids: vec![151643, 151645],
            pad_token_id: 151643,
            im_start_token_id: 151644,
            im_end_token_id: 151645,
            audio_start_token_id: 151669,
            audio_end_token_id: 151670,
            audio_pad_token_id: 151676,
            asr_text_token_id: 151704,
        }
    }

    #[test]
    fn encoder_output_lengths() {
        assert_eq!(get_feat_extract_output_lengths(0), 0);
        assert_eq!(get_feat_extract_output_lengths(1), 1);
        assert_eq!(get_feat_extract_output_lengths(100), 13);
        assert_eq!(get_feat_extract_output_lengths(200), 26);
        assert_eq!(get_feat_extract_output_lengths(997), 130);
    }

    #[test]
    fn prompt_structure() {
        let st = test_special_tokens();
        let ids = build_prompt_ids(&st, 10, None);
        assert_eq!(ids.len(), 25); // 15 fixed + 10 audio_pad
        assert_eq!(ids[0], st.im_start_token_id);
        assert_eq!(ids[8], st.audio_start_token_id);
        for (offset, id) in ids[9..19].iter().enumerate() {
            assert_eq!(
                *id,
                st.audio_pad_token_id,
                "audio_pad at index {}",
                9 + offset
            );
        }
        assert_eq!(ids[19], st.audio_end_token_id);
        assert_eq!(ids[24], NEWLINE_TOKEN_ID);
        assert_eq!(get_audio_pad_start(&ids, st.audio_pad_token_id), Some(9));
    }

    #[test]
    fn prompt_with_language_hint() {
        let st = test_special_tokens();
        let ids = build_prompt_ids(&st, 5, Some(&[6364]));
        assert_eq!(ids.len(), 23); // 20 standard + 3 language hint
        assert_eq!(ids[20], LANGUAGE_TOKEN_ID);
        assert_eq!(ids[21], 6364);
        assert_eq!(ids[22], st.asr_text_token_id);
    }
}
