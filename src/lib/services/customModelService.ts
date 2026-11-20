// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Custom Model Service - thin invoke wrappers for the BYO custom model
 * commands. Mirrors Rust `commands::custom_model`.
 */

import { writable, get } from 'svelte/store';
import { mustInvoke } from '$lib/ipc/invoke';
import { modelStore } from '$lib/stores/modelStore';
import type {
  CustomModelEntry,
  CustomModelError,
  CustomModelInfo,
  HfOnnxCandidate,
} from '$lib/types/models';

/**
 * Bumped after every import/replace/remove so all surfaces showing the custom
 * model (settings section, import modal, picker rows) can refresh live, even
 * while another surface performed the change.
 */
export const customModelChanged = writable(0);

/**
 * Refresh the model store's downloaded-models cache BEFORE announcing the
 * change: `selectModel('custom')` computes `modelExists` from that cache, and
 * a stale entry right after the first import made the bottom bar fall back to
 * "Select AI Model" instead of showing the imported model as active.
 */
async function notifyChanged(): Promise<void> {
  try {
    await modelStore.refreshDownloadedModels();
  } catch (error) {
    console.error('Failed to refresh downloaded models after custom model change:', error);
  }
  customModelChanged.update(n => n + 1);
}

/**
 * Parse the JSON-encoded typed error the Rust commands return. Anything
 * unparseable becomes an `Internal` error with the raw text as message.
 */
export function parseCustomModelError(error: unknown): CustomModelError {
  if (typeof error === 'string') {
    try {
      const parsed = JSON.parse(error) as CustomModelError;
      if (parsed && typeof parsed.code === 'string') {
        return parsed;
      }
    } catch {
      // Fall through to the generic wrapper below.
    }
    return { code: 'Internal', message: error };
  }
  if (error instanceof Error) {
    return { code: 'Internal', message: error.message };
  }
  return { code: 'Internal', message: String(error) };
}

/** Import a custom ONNX model from a local file path. */
export async function importCustomModelFromFile(
  sourcePath: string,
  displayName: string,
  companionDataPath?: string
): Promise<CustomModelInfo> {
  const info = await mustInvoke<CustomModelInfo>('import_custom_model_from_file', {
    sourcePath,
    companionDataPath: companionDataPath ?? null,
    displayName,
  });
  await notifyChanged();
  return info;
}

/**
 * List every `.onnx` file in a HuggingFace repo, resolving ambiguity for the
 * caller. When `url` already points at one exact file (a `/blob/...` or
 * `/resolve/...` link), returns just that file (validated to exist) instead
 * of the whole repo's `.onnx` files.
 */
export async function listHfOnnxFiles(url: string): Promise<HfOnnxCandidate[]> {
  return mustInvoke<HfOnnxCandidate[]>('list_hf_onnx_files', { url });
}

/** Import one exact `.onnx` file from a HuggingFace repo, resolved beforehand
 * via `listHfOnnxFiles` (either the repo's single file or the user's pick). */
export async function importCustomModelFromUrl(
  url: string,
  filePath: string
): Promise<CustomModelInfo> {
  const info = await mustInvoke<CustomModelInfo>('import_custom_model_from_url', {
    url,
    filePath,
  });
  await notifyChanged();
  return info;
}

/**
 * Delete one imported custom model by its directory path. When it was the
 * SELECTED custom model and that model was active, the backend falls back to
 * the default model - mirror that in the frontend store so the picker/bottom
 * bar don't keep claiming a model that no longer exists.
 */
export async function removeCustomModel(path: string): Promise<void> {
  await mustInvoke('remove_custom_model', { path });
  await notifyChanged();
  if (get(modelStore).selectedModel === 'custom') {
    try {
      const settings = await mustInvoke<{ model: string }>('get_settings');
      if (settings.model && settings.model !== 'custom') {
        await modelStore.selectModel(settings.model);
      }
    } catch (error) {
      console.error('Failed to sync model selection after custom model removal:', error);
    }
  }
}

/** Absolute directory path for a BYO download in progress (before it has a
 * `list_custom_models` entry to read a path from) - used by the "open
 * download folder" button in the shared download dialog. */
export async function getCustomModelDirPath(modelDirName: string): Promise<string> {
  return mustInvoke<string>('get_custom_model_dir_path', { modelDirName });
}

/** Read the SELECTED custom model's metadata, or null when nothing is selected. */
export async function getCustomModelInfo(): Promise<CustomModelInfo | null> {
  return (await mustInvoke<CustomModelInfo | null>('get_custom_model_info')) ?? null;
}

/** List every imported custom model, newest first. */
export async function listCustomModels(): Promise<CustomModelEntry[]> {
  return mustInvoke<CustomModelEntry[]>('list_custom_models');
}

/** Make an already-imported model the selected custom model. */
export async function selectCustomModel(path: string): Promise<CustomModelInfo> {
  const info = await mustInvoke<CustomModelInfo>('select_custom_model', { path });
  await notifyChanged();
  return info;
}
