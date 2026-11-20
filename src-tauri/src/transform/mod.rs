// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Transform module for post-processing transcription output
//!
//! Uses local ONNX LLMs to fix grammar, spelling, and punctuation.

pub mod opencc;
mod t5_grammar;

pub use t5_grammar::T5GrammarCorrector;

/// Apply the user's `chinese_script` setting to transcription output.
/// No-op for text without Han characters.
pub fn convert_chinese_script(text: String, setting: &str) -> String {
    opencc::convert(text, opencc::ChineseScript::from_setting(setting))
}
