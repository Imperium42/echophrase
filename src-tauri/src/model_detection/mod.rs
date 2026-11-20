// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Auto-detection module for selecting optimal transcription model based on GPU VRAM.
//!
//! This module runs on first launch to select the best model the user's hardware can support.
//! Isolated for testability and single-responsibility.
//!
//! # Architecture
//!
//! - `vram_analyzer`: Classifies GPU VRAM into tiers
//! - `model_selector`: Selects optimal model based on tier
//!
//! # Usage
//!
//! ```rust,ignore
//! use model_detection::{select_optimal_model, VramTier};
//!
//! let selection = select_optimal_model(Some(8192)); // 8GB VRAM
//! println!("Selected: {} ({})", selection.model_id, selection.reason);
//! ```

mod model_selector;
mod vram_analyzer;

#[allow(unused_imports)]
pub use model_selector::{
    ModelSelection, get_compatible_models, is_non_english_system_locale, model_ids,
    select_optimal_model, system_prefers_simplified_chinese,
};
pub use vram_analyzer::VramTier;
