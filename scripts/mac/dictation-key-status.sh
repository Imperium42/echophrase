#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Show the current hidutil key mapping so you can confirm whether the
# dictation-key -> F13 remap is active.
set -euo pipefail

echo "Current hidutil UserKeyMapping:"
hidutil property --get "UserKeyMapping"
echo
echo "If active you'll see Src 0xc000000cf -> Dst 0x700000068 (F13)."
