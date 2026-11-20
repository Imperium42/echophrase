#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Revert the dictation-key -> F13 remap set by dictation-key-remap.sh, restoring
# the macOS built-in Dictation behaviour on the mic key.
#
# NOTE: this clears the ENTIRE hidutil UserKeyMapping, so it also removes any
# other manual hidutil remaps from this session. (The app does the same on quit.)
set -euo pipefail

hidutil property --set '{"UserKeyMapping":[]}' >/dev/null
echo "Cleared hidutil UserKeyMapping - dictation key restored to system Dictation."
