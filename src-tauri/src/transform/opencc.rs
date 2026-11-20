// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Simplified <-> Traditional Chinese script conversion (OpenCC lexicons).
//!
//! Applied to ASR output right after transcription, before the (English-only)
//! grammar transform. User-controlled via the `chinese_script` setting:
//! `"traditional"` (s2t, default) or `"simplified"` (t2s).

use opencc_fmmseg::OpenCC;
use std::sync::LazyLock;

/// Embedded-dictionary OpenCC converter, built once on first use.
static OPENCC: LazyLock<OpenCC> = LazyLock::new(OpenCC::new);

/// Target Chinese script for transcription output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChineseScript {
    /// Convert Simplified -> Traditional (s2t).
    Traditional,
    /// Convert Traditional -> Simplified (t2s).
    Simplified,
}

impl ChineseScript {
    /// Parse the `chinese_script` settings string; unknown values (incl. the
    /// removed legacy "as_is") fall back to the Traditional default.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "simplified" => Self::Simplified,
            _ => Self::Traditional,
        }
    }
}

/// Convert `text` to the requested Chinese script. Fast no-op when the text
/// contains no Han characters.
pub fn convert(text: String, script: ChineseScript) -> String {
    let config = match script {
        ChineseScript::Traditional => "s2t",
        ChineseScript::Simplified => "t2s",
    };
    if !contains_han(&text) {
        return text;
    }
    OPENCC.convert(&text, config, false)
}

/// True if any char is in the CJK Unified Ideographs blocks (BMP + Ext A).
pub fn contains_han(text: &str) -> bool {
    text.chars().any(is_han)
}

/// True when the majority of non-whitespace chars are Han - used to skip the
/// English-only grammar transform for Chinese output.
pub fn is_majority_han(text: &str) -> bool {
    let mut han = 0usize;
    let mut total = 0usize;
    for c in text.chars() {
        if c.is_whitespace() {
            continue;
        }
        total += 1;
        if is_han(c) {
            han += 1;
        }
    }
    total > 0 && han * 2 > total
}

#[inline]
fn is_han(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s2t_converts_simplified_to_traditional() {
        let out = convert("汉字转换测试".to_string(), ChineseScript::Traditional);
        assert_eq!(out, "漢字轉換測試");
    }

    #[test]
    fn t2s_converts_traditional_to_simplified() {
        let out = convert("漢字轉換測試".to_string(), ChineseScript::Simplified);
        assert_eq!(out, "汉字转换测试");
    }

    #[test]
    fn ascii_fast_path_is_identity() {
        let input = "Hello, world! 123".to_string();
        assert_eq!(
            convert(input.clone(), ChineseScript::Traditional),
            input.clone()
        );
        assert_eq!(convert(input.clone(), ChineseScript::Simplified), input);
    }

    #[test]
    fn setting_parse() {
        assert_eq!(
            ChineseScript::from_setting("traditional"),
            ChineseScript::Traditional
        );
        assert_eq!(
            ChineseScript::from_setting("simplified"),
            ChineseScript::Simplified
        );
        // Legacy "as_is" and unknown values fall back to the default.
        assert_eq!(
            ChineseScript::from_setting("as_is"),
            ChineseScript::Traditional
        );
        assert_eq!(
            ChineseScript::from_setting("bogus"),
            ChineseScript::Traditional
        );
    }

    #[test]
    fn majority_han_detection() {
        assert!(is_majority_han("你好世界"));
        assert!(is_majority_han("我想要喝一大杯 coffee"));
        assert!(!is_majority_han("Hello world 你好"));
        assert!(!is_majority_han(""));
        assert!(!is_majority_han("Plain English sentence."));
    }
}
