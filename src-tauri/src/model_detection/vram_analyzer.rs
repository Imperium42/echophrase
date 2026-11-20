// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! VRAM analysis and tier classification for model auto-detection.
//!
//! This module provides utilities to classify GPU VRAM capacity into tiers
//! that map to compatible transcription models.

use serde::Serialize;

/// VRAM capacity tiers for model selection.
///
/// Each tier maps to a set of compatible models based on their VRAM requirements.
/// Tiers are ordered from lowest to highest capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VramTier {
    /// No GPU detected or VRAM unknown - use smallest model for CPU fallback
    None,
    /// < 2GB VRAM - insufficient for most models
    Insufficient,
    /// 2-4GB VRAM - small models only
    Low,
    /// 4-6GB VRAM - medium/distil models
    Medium,
    /// 6-8GB VRAM - large models with headroom
    High,
    /// 8GB+ VRAM - any model, prefer highest quality
    Premium,
}

impl VramTier {
    /// Classify VRAM amount into a tier.
    ///
    /// # Arguments
    /// * `vram_mb` - VRAM in megabytes, None if unknown/unavailable
    ///
    /// # Returns
    /// The appropriate `VramTier` for the given VRAM capacity
    pub fn from_vram_mb(vram_mb: Option<u32>) -> Self {
        match vram_mb {
            None => VramTier::None,
            Some(mb) if mb < 2000 => VramTier::Insufficient,
            Some(mb) if mb < 4000 => VramTier::Low,
            Some(mb) if mb < 6000 => VramTier::Medium,
            Some(mb) if mb < 8000 => VramTier::High,
            Some(_) => VramTier::Premium,
        }
    }

    /// Get human-readable description of the tier
    pub fn description(&self) -> &'static str {
        match self {
            VramTier::None => "No GPU detected",
            VramTier::Insufficient => "Less than 2GB VRAM",
            VramTier::Low => "2-4GB VRAM",
            VramTier::Medium => "4-6GB VRAM",
            VramTier::High => "6-8GB VRAM",
            VramTier::Premium => "8GB+ VRAM",
        }
    }
}

impl std::fmt::Display for VramTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vram_tier_none() {
        assert_eq!(VramTier::from_vram_mb(None), VramTier::None);
    }

    #[test]
    fn test_vram_tier_insufficient() {
        assert_eq!(VramTier::from_vram_mb(Some(0)), VramTier::Insufficient);
        assert_eq!(VramTier::from_vram_mb(Some(1024)), VramTier::Insufficient);
        assert_eq!(VramTier::from_vram_mb(Some(1999)), VramTier::Insufficient);
    }

    #[test]
    fn test_vram_tier_low() {
        assert_eq!(VramTier::from_vram_mb(Some(2000)), VramTier::Low);
        assert_eq!(VramTier::from_vram_mb(Some(2048)), VramTier::Low);
        assert_eq!(VramTier::from_vram_mb(Some(3072)), VramTier::Low);
        assert_eq!(VramTier::from_vram_mb(Some(3999)), VramTier::Low);
    }

    #[test]
    fn test_vram_tier_medium() {
        assert_eq!(VramTier::from_vram_mb(Some(4000)), VramTier::Medium);
        assert_eq!(VramTier::from_vram_mb(Some(4096)), VramTier::Medium);
        assert_eq!(VramTier::from_vram_mb(Some(5120)), VramTier::Medium);
        assert_eq!(VramTier::from_vram_mb(Some(5999)), VramTier::Medium);
    }

    #[test]
    fn test_vram_tier_high() {
        assert_eq!(VramTier::from_vram_mb(Some(6000)), VramTier::High);
        assert_eq!(VramTier::from_vram_mb(Some(6144)), VramTier::High);
        assert_eq!(VramTier::from_vram_mb(Some(7168)), VramTier::High);
        assert_eq!(VramTier::from_vram_mb(Some(7999)), VramTier::High);
    }

    #[test]
    fn test_vram_tier_premium() {
        assert_eq!(VramTier::from_vram_mb(Some(8000)), VramTier::Premium);
        assert_eq!(VramTier::from_vram_mb(Some(8192)), VramTier::Premium);
        assert_eq!(VramTier::from_vram_mb(Some(12288)), VramTier::Premium);
        assert_eq!(VramTier::from_vram_mb(Some(24576)), VramTier::Premium);
    }

    #[test]
    fn test_vram_tier_ordering() {
        assert!(VramTier::None < VramTier::Insufficient);
        assert!(VramTier::Insufficient < VramTier::Low);
        assert!(VramTier::Low < VramTier::Medium);
        assert!(VramTier::Medium < VramTier::High);
        assert!(VramTier::High < VramTier::Premium);
    }
}
