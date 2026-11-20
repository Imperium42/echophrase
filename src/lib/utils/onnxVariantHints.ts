// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Recognizes common ONNX export naming conventions (quantization/precision
 * suffixes) so the HuggingFace multi-file picker can explain what each
 * candidate actually is, instead of leaving the user to guess from a bare
 * filename like `model.int8.onnx`.
 *
 * Order matters: more specific patterns (eg `q4f16`) must be checked before
 * the generic ones they'd otherwise also match (eg `q4`, `fp16`).
 */

import { translate } from '$lib/locales';

type VariantPattern = {
  test: RegExp;
  key: string;
};

const PATTERNS: VariantPattern[] = [
  { test: /q4f16/i, key: 'q4f16' },
  { test: /(^|[._-])q4($|[._-])/i, key: 'q4' },
  { test: /int8/i, key: 'int8' },
  { test: /(fp16|float16)/i, key: 'fp16' },
  { test: /quantized/i, key: 'quantized' },
  { test: /merged/i, key: 'merged' },
];

/** Look up the i18n explanation for a recognized filename pattern, or `null`. */
export function describeOnnxVariant(filename: string): string | null {
  const match = PATTERNS.find(p => p.test.test(filename));
  return match ? translate(`customModel.variantHints.${match.key}`) : null;
}
