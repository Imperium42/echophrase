// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * AI Model Configuration Types and Registry
 * Centralized model definitions supporting multiple backends (Candle, ONNX)
 */

// ============================================================================
// Feature Flags
// ============================================================================

/**
 * ONNX-Only Mode: When true, hides safetensors models (Candle backend)
 *
 * Why: Candle with CUDA links cudart at compile time, putting it in the PE
 * import table. This causes Windows to crash before main() if CUDA DLLs are missing.
 * ONNX Runtime with load-dynamic loads CUDA at runtime, allowing graceful fallback.
 *
 * Set to false to re-enable Whisper safetensors models (requires bundled CUDA DLLs).
 */
export const ONNX_ONLY_MODE = true;

// ============================================================================
// Enums
// ============================================================================

/** Model backend/runtime type */
export type ModelBackend = 'candle' | 'onnx';

/** Model provider/source */
export type ModelProvider = 'whisper' | 'nvidia' | 'qwen' | 'custom';

/** Model file format */
export type ModelFormat = 'safetensors' | 'onnx' | 'gguf';

/** Which UI surface lists this model. Mirrors Rust `ModelCategory`. */
export type ModelCategory = 'transcription' | 'transformation';

/** Model size category for UI display */
export type SizeCategory = 'tiny' | 'small' | 'medium' | 'large' | 'xlarge';

/** Model download status states */
export enum DownloadStatus {
  Connecting = 'connecting',
  Downloading = 'downloading',
  VerifyingIntegrity = 'verifying',
  Complete = 'complete',
  Error = 'error',
  Cancelled = 'cancelled',
}

/** View states for model download dialog */
export enum ModelDialogState {
  Selection = 'selection',
  Ready = 'ready',
  Downloading = 'downloading',
  Success = 'success',
  Error = 'error',
  Info = 'info',
  ConfirmDelete = 'confirm-delete',
}

// ============================================================================
// Interfaces
// ============================================================================

/** Complete model definition for the registry */
export interface ModelDefinition {
  /** Unique model identifier */
  id: string;
  /** Display name for UI */
  name: string;
  /** Short description (non-redundant with stats) */
  description: string;
  /** Optional extra detail for (?) tooltip */
  hint?: string;
  /** Model provider/source */
  provider: ModelProvider;
  /** Which UI lists this model (transcription picker vs transformation picker) */
  category: ModelCategory;
  /** Backend/runtime to use */
  backend: ModelBackend;
  /** Model file format */
  format: ModelFormat;
  /** Size in bytes */
  sizeBytes: number;
  /** Size category for badges */
  sizeCategory: SizeCategory;
  /** Realtime speed multiplier (e.g., "6x") */
  realtimeSpeed: string;
  /** Minimum VRAM required */
  minVram: string;
  /** GPU recommendation */
  gpuRecommendation: string;
  /** HuggingFace repo ID or download URL */
  repoId: string;
  /** Number of model parameters */
  parameters?: string;
  /** Language support (short label, e.g. "25 European") */
  languages?: string;
  /** Full supported-language list for the multi-language badge tooltip.
   *  Presence of this field renders the badge next to the model name. */
  languagesFull?: string;
  /** NEW badge - recently added */
  isNew?: boolean;
  /** RECOMMENDED badge - best default choice */
  isRecommended?: boolean;
  /** Experimental maturity flag - shows an "Alpha" badge. */
  isAlpha?: boolean;
  /** Coming soon - not yet available */
  isComingSoon?: boolean;
  /** Runtime download status (set dynamically) */
  isDownloaded?: boolean;
  /** Ships in the app bundle - never deletable, always present. Mirrors Rust `ModelRegistryEntry::bundled`. */
  bundled?: boolean;
  /** Model supports Eastern languages (Chinese, Japanese, Korean, etc). False/undefined = European/English only. */
  supportsEasternLanguages?: boolean;
  /** Model consumes the `language` setting as a transcription hint (auto vs forced language). */
  supportsLanguageHint?: boolean;
}

/** Model download progress interface */
export interface ModelDownloadProgress {
  progress: number; // 0-100
  speed: string;
  timeRemaining: string;
  timeElapsed: string;
  status: DownloadStatus;
}

/** Model selection event interface */
export interface ModelSelectionEvent {
  modelId: string;
  isDownloaded: boolean;
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Get size category from bytes */
export function getSizeCategory(bytes: number): SizeCategory {
  if (bytes < 100_000_000) return 'tiny'; // < 100 MB
  if (bytes < 500_000_000) return 'small'; // 100 MB - 500 MB
  if (bytes < 1_500_000_000) return 'medium'; // 500 MB - 1.5 GB
  if (bytes < 3_000_000_000) return 'large'; // 1.5 GB - 3 GB
  return 'xlarge'; // > 3 GB
}

/** Get human-readable size string */
export function formatSize(bytes: number): string {
  if (bytes < 1_000_000) return `${Math.round(bytes / 1000)} KB`;
  if (bytes < 1_000_000_000) return `${Math.round(bytes / 1_000_000)} MB`;
  return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
}

/** Get size category badge color (Tailwind class) */
export function getSizeBadgeColor(category: SizeCategory): string {
  const colors: Record<SizeCategory, string> = {
    tiny: 'bg-zinc-500/20 text-zinc-400',
    small: 'bg-blue-500/20 text-blue-400',
    medium: 'bg-amber-500/20 text-amber-400',
    large: 'bg-purple-500/20 text-purple-400',
    xlarge: 'bg-red-500/20 text-red-400',
  };
  return colors[category];
}

// ============================================================================
// Model Registry
// ============================================================================

export const MODEL_REGISTRY: ModelDefinition[] = [
  // ─────────────────────────────────────────────────────────────────────────
  // Whisper template (Candle backend, safetensors) - disabled.
  //
  // The desktop build uses ONNX-only for runtime portability (see
  // ONNX_ONLY_MODE above). Keep this block as the canonical template for
  // any future safetensors/Candle model - copy, rename, swap repoId.
  // ─────────────────────────────────────────────────────────────────────────
  // {
  //   id: 'distil-large-v3',
  //   name: 'Distil-Whisper Large-v3',
  //   description: 'Best balance of speed and accuracy',
  //   hint: 'Knowledge-distilled architecture optimizes for inference speed without sacrificing transcription quality.',
  //   provider: 'whisper',
  //   backend: 'candle',
  //   format: 'safetensors',
  //   sizeBytes: 1_500_000_000,
  //   sizeCategory: 'medium',
  //   realtimeSpeed: '6x',
  //   minVram: '4GB',
  //   gpuRecommendation: 'RTX 3070+',
  //   repoId: 'distil-whisper/distil-large-v3',
  //   parameters: '756M',
  //   languages: '99 languages',
  //   isRecommended: true,
  // },

  // ─────────────────────────────────────────────────────────────────────────
  // NVIDIA Models (ONNX backend)
  // ─────────────────────────────────────────────────────────────────────────
  {
    id: 'parakeet-tdt-0.6b-v3',
    name: 'Parakeet-TDT 0.6B',
    description: 'Auto language detection, ONNX runtime',
    hint: 'State-of-the-art ASR. Supports 25 European languages with automatic detection.',
    provider: 'nvidia',
    category: 'transcription',
    backend: 'onnx',
    format: 'onnx',
    sizeBytes: 2_550_000_000,
    sizeCategory: 'large',
    realtimeSpeed: '12x',
    minVram: '6GB',
    gpuRecommendation: 'RTX 3060+',
    repoId: 'istupakov/parakeet-tdt-0.6b-v3-onnx',
    parameters: '600M',
    languages: '25 EU Languages',
    languagesFull:
      'Bulgarian, Croatian, Czech, Danish, Dutch, English, Estonian, Finnish, French, German, Greek, Hungarian, Italian, Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Russian, Slovak, Slovenian, Spanish, Swedish, Ukrainian',
    isRecommended: true,
    supportsEasternLanguages: false,
  },
  {
    id: 'parakeet-eou-120m',
    name: 'Parakeet Lite 120M',
    description: 'Lightweight, runs on any hardware',
    hint: "NVIDIA's smallest ASR model. English only. Designed for real-time on low-power devices. Uses GPU if available, CPU otherwise.",
    provider: 'nvidia',
    category: 'transcription',
    backend: 'onnx',
    format: 'onnx',
    sizeBytes: 481_000_000,
    sizeCategory: 'small',
    realtimeSpeed: '2-5x',
    minVram: 'None (GPU optional)',
    gpuRecommendation: 'Potato',
    repoId: 'altunenes/parakeet-rs',
    parameters: '120M',
    languages: 'English',
    supportsEasternLanguages: false,
  },

  // ─────────────────────────────────────────────────────────────────────────
  // Qwen3-ASR (ONNX backend) - international / Eastern languages
  // Mirrors Rust MODEL_REGISTRY "qwen3-asr-0.6b".
  // ─────────────────────────────────────────────────────────────────────────
  {
    id: 'qwen3-asr-0.6b',
    name: 'Qwen3-ASR 0.6B',
    description: 'International: Chinese + 52 languages',
    hint: "Alibaba's multilingual ASR (Apache-2.0). 52 languages including Chinese dialects and mixed English/Chinese code-switching. int4-quantized decoder.",
    provider: 'qwen',
    category: 'transcription',
    backend: 'onnx',
    format: 'onnx',
    sizeBytes: 2_030_000_000,
    sizeCategory: 'large',
    realtimeSpeed: '2-6x',
    minVram: '4GB',
    gpuRecommendation: 'RTX 3060+',
    repoId: 'andrewleech/qwen3-asr-0.6b-onnx',
    parameters: '0.6B',
    languages: '52 EU/APAC Languages',
    languagesFull:
      'Arabic, Chinese (Mandarin + 22 dialects incl. Cantonese, Hokkien, Sichuanese), Czech, Danish, Dutch, English (multiple accents), Filipino, Finnish, French, German, Greek, Hindi, Hungarian, Indonesian, Italian, Japanese, Korean, Macedonian, Malay, Persian, Polish, Portuguese, Romanian, Russian, Spanish, Swedish, Thai, Turkish, Vietnamese',
    isNew: true,
    supportsEasternLanguages: true,
    supportsLanguageHint: true,
  },

  // ─────────────────────────────────────────────────────────────────────────
  // Transformation models (T5-family grammar correctors)
  // Mirrors Rust MODEL_REGISTRY transformation entries.
  // ─────────────────────────────────────────────────────────────────────────
  {
    id: 't5-mini-grammar',
    name: 'T5-Mini Grammar',
    description: 'Corrects grammar - better for multiple, longer sentences.',
    hint: 'Quantized T5-Mini fine-tuned on the C4_200M dataset. ~31M params, runs instantly on any modern CPU. MIT-licensed, commercially safe.',
    provider: 'custom',
    category: 'transformation',
    backend: 'onnx',
    format: 'onnx',
    sizeBytes: 60_000_000,
    sizeCategory: 'tiny',
    realtimeSpeed: 'fast',
    minVram: 'Potato',
    gpuRecommendation: 'Any GPU',
    repoId: 'visheratin/t5-efficient-mini-grammar-correction',
    parameters: '31M',
    languages: 'English',
    isRecommended: true,
  },
  {
    id: 'custom-transform',
    name: 'Custom ONNX Model',
    description: 'Bring your own ONNX grammar/text model',
    hint: 'Drop in a compatible T5-family ONNX file. Licensing is your call - same posture as LM Studio / Ollama: the runtime is generic, the model choice is yours.',
    provider: 'custom',
    category: 'transformation',
    backend: 'onnx',
    format: 'onnx',
    sizeBytes: 0,
    sizeCategory: 'tiny',
    realtimeSpeed: 'varies',
    minVram: 'varies',
    gpuRecommendation: 'varies',
    repoId: '',
    isComingSoon: true,
  },

  // ─────────────────────────────────────────────────────────────────────────
  // Custom Models - BYO ONNX transcription model.
  // Imported via Settings > Custom Model (local file or HuggingFace repo),
  // validated + stored by the Rust `custom_model` commands, and run through
  // OnnxRuntimeBackend's generic inference path.
  // TODO(safetensors): accept .safetensors imports again once the Candle
  // backend returns (ONNX_ONLY_MODE lifted).
  // ─────────────────────────────────────────────────────────────────────────
  {
    id: 'custom',
    name: 'Custom ONNX Models',
    description: 'Bring your own ONNX model',
    provider: 'custom',
    category: 'transcription',
    backend: 'onnx',
    format: 'onnx',
    sizeBytes: 0,
    sizeCategory: 'medium',
    realtimeSpeed: 'varies',
    minVram: 'varies',
    gpuRecommendation: 'varies',
    repoId: '',
    isAlpha: true,
  },
];

// ============================================================================
// BYO Custom Model types - mirror Rust `commands::custom_model` /
// `models::byo_validator`. Keep in sync.
// ============================================================================

/** Metadata for an imported BYO custom model (Rust `CustomModelInfo`). */
export interface CustomModelInfo {
  name: string;
  /** "onnx" in v1. */
  format: string;
  sizeBytes: number;
  /** "file" or a HuggingFace repo id like "org/repo". */
  source: string;
  originalFilename: string;
  /** RFC3339 timestamp. */
  importedAt: string;
  isLoadable: boolean;
  /** Canonical source file path for local imports (null/absent for HF imports
   * and pre-existing models). Backend-only dedupe key, display never uses it. */
  sourcePath?: string | null;
}

/** A discovered imported model + its managed directory (Rust `CustomModelEntry`).
 * The path is the stable identifier passed to select/remove. */
export interface CustomModelEntry {
  info: CustomModelInfo;
  path: string;
}

/** One `.onnx` file found in a HuggingFace repo (Rust `HfOnnxCandidate`),
 * offered as an import choice when a repo has more than one. */
export interface HfOnnxCandidate {
  path: string;
  sizeBytes: number;
}

/** Typed error codes from BYO import commands (Rust `ByoErrorCode`). */
export type CustomModelErrorCode =
  | 'UnsupportedFormat'
  | 'FileTooLarge'
  | 'CorruptOrInvalid'
  | 'SymlinkRejected'
  | 'HfRepoAmbiguous'
  | 'HfRepoNotFound'
  | 'NetworkError'
  | 'PathTraversalRejected'
  | 'NotImported'
  | 'DuplicateSource'
  | 'DownloadInProgress'
  | 'Internal';

/** Structured error thrown by BYO import commands. */
export interface CustomModelError {
  code: CustomModelErrorCode;
  message: string;
}

// ============================================================================
// Registry Helpers
// ============================================================================

/** Get model by ID */
export function getModelById(id: string): ModelDefinition | undefined {
  return MODEL_REGISTRY.find(m => m.id === id);
}

/**
 * Get available models (excluding coming soon, respects ONNX_ONLY_MODE).
 *
 * @param ortAvailable - Whether ONNX Runtime can load. On macOS < 13.3, ORT
 *   fails due to missing C++20 symbols, so ONNX models are excluded and
 *   Candle/safetensors models are shown instead (overriding ONNX_ONLY_MODE).
 * @param category - Optional filter -defaults to 'transcription' to preserve
 *   existing call-site behavior. Pass 'transformation' for grammar models.
 */
export function getAvailableModels(
  ortAvailable: boolean = true,
  category: ModelCategory = 'transcription'
): ModelDefinition[] {
  let models = MODEL_REGISTRY.filter(m => !m.isComingSoon && m.category === category);

  if (!ortAvailable) {
    // ORT unavailable (e.g. macOS < 13.3) - keep the full list rather than
    // filtering to an empty one. ONNX models are flagged incompatible by
    // isModelCompatible() so the picker can grey them out with its usual
    // messaging; ONNX_ONLY_MODE is skipped because it would hide Candle models.
  } else if (ONNX_ONLY_MODE) {
    models = models.filter(m => m.format === 'onnx');
  }

  return models;
}

/** Get recommended model for a category (respects ONNX_ONLY_MODE) */
export function getRecommendedModel(
  category: ModelCategory = 'transcription'
): ModelDefinition | undefined {
  const available = getAvailableModels(true, category);
  // First try to find an explicitly recommended model
  const recommended = available.find(m => m.isRecommended);
  if (recommended) return recommended;
  // Fallback to first available model (e.g., when ONNX_ONLY_MODE hides recommended)
  return available[0];
}

/** Get models by backend */
export function getModelsByBackend(backend: ModelBackend): ModelDefinition[] {
  return MODEL_REGISTRY.filter(m => m.backend === backend);
}

/** Get models by provider */
export function getModelsByProvider(provider: ModelProvider): ModelDefinition[] {
  return MODEL_REGISTRY.filter(m => m.provider === provider);
}

/** Default model ID (respects ONNX_ONLY_MODE) */
export const DEFAULT_MODEL_ID = 'parakeet-tdt-0.6b-v3';

// ============================================================================
// Legacy Compatibility (for existing code)
// ============================================================================

/** @deprecated Use MODEL_REGISTRY instead */
export enum WhisperModelSize {
  Tiny = 'tiny',
  Base = 'base',
  Medium = 'whisper-medium',
  Large = 'large',
  LargeV2 = 'large-v2',
  LargeV3 = 'distil-large-v3',
}

/** @deprecated Use getModelById instead */
export function getModelDisplayInfo(modelId: string): {
  name: string;
  size: string;
  quality: string;
  gpu: string;
} {
  const model = getModelById(modelId);
  if (!model) {
    return { name: modelId, size: 'Unknown', quality: 'Unknown', gpu: 'Unknown' };
  }
  return {
    name: model.name,
    size: formatSize(model.sizeBytes),
    quality: model.description,
    gpu: model.gpuRecommendation,
  };
}
