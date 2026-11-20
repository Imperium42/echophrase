#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Remap the dedicated macOS Dictation (F5 / mic-icon) key to F13 so an app can
# bind it as a normal global shortcut, WITHOUT the user disabling system
# Dictation in System Settings.
#
# The dictation key is HID Consumer-page usage 0xc000000cf ("Voice Command").
# F13 is Keyboard-page usage 0x700000068. The remap is session-only: it takes
# effect on the next keypress and is cleared on reboot. Run dictation-key-revert.sh
# (or reboot) to undo.
#
# The app does this itself via hidutil at launch/quit (src-tauri/src/macos_dictation.rs);
# this script is for manual testing / debugging the remap in isolation.
set -euo pipefail

SRC="0xc000000cf"   # dictation / Voice Command key
DST="0x700000068"   # F13

hidutil property --set \
  "{\"UserKeyMapping\":[{\"HIDKeyboardModifierMappingSrc\":${SRC},\"HIDKeyboardModifierMappingDst\":${DST}}]}" \
  >/dev/null

echo "Remapped dictation key (${SRC}) -> F13 (${DST})."
echo "Press the mic key; it should now register as F13 (no system Dictation)."
echo "Undo with: scripts/mac/dictation-key-revert.sh  (or reboot)"
