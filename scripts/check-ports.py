# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

"""Probe which candidate dev ports are actually bindable on this Windows box.
Hyper-V reserves port ranges invisibly; the only reliable detection is to try binding.
"""
import socket

CANDIDATES = [
    # Current (to confirm breakage)
    1420, 1421,
    # Project ports referenced elsewhere
    3042, 3044, 5042,
    # Vite default + fallbacks
    5173, 5174, 5175,
    # Safer high-range picks (above common Hyper-V reservation zones, below ephemeral 49152)
    4420, 4421, 8420, 8421, 9420, 9421, 17420, 17421, 24420, 24421,
]

# Prod CORS (Coolify CORS_ORIGINS) only allows these dev origins; see CLAUDE.md "Ports".
PROD_CORS_ALLOWED = {1420, 5173, 8420}


def probe(port: int) -> str:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        s.bind(("127.0.0.1", port))
        s.close()
        return "OK"
    except OSError as e:
        return f"BLOCKED ({e.winerror if hasattr(e, 'winerror') else e.errno})"

width = max(len(str(p)) for p in CANDIDATES)
for p in CANDIDATES:
    tag = "  <- prod CORS allows this" if p in PROD_CORS_ALLOWED else ""
    print(f"  {str(p).rjust(width)} : {probe(p)}{tag}")
