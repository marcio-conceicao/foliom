#!/usr/bin/env bash
# scripts/footprint_check.sh
# Installer size gate for DSK-03.
#
# Usage: bash scripts/footprint_check.sh <installer_path> [ceiling_mb]
#
# Args:
#   installer_path  — path to .dmg or .msi/.exe installer artifact
#   ceiling_mb      — size ceiling in MB (default: 30)
#
# Exit codes:
#   0 = installer size is within budget
#   1 = installer not found, or size exceeds ceiling
#
# Called from .github/workflows/release.yml after tauri-action completes
# (macOS only — Windows uses inline PowerShell; see release.yml).
#
# Security note (T-05-12): This script is called from CI with a hard-coded
# glob path. The installer_path argument is never sourced from user input in
# the production CI path. The [ ! -f ] guard rejects missing or unreadable files.

set -euo pipefail

INSTALLER="${1:?Usage: footprint_check.sh <installer_path> [ceiling_mb]}"
CEILING="${2:-30}"

if [ ! -f "$INSTALLER" ]; then
    echo "::error::Installer not found: $INSTALLER"
    exit 1
fi

# du -sm: reports size in megabytes (1 MB = 1024*1024 on macOS/BSD, 1000*1000 on GNU).
# The ~5% difference between BSD and GNU du is within the headroom for a
# ~10–15 MB Tauri installer vs the 30 MB ceiling. Portable across macOS and Linux.
SIZE_MB=$(du -sm "$INSTALLER" | cut -f1)

echo "Installer: $INSTALLER"
echo "Size:      ${SIZE_MB} MB  (ceiling: ${CEILING} MB)  [DSK-03]"

if [ "$SIZE_MB" -gt "$CEILING" ]; then
    echo "::error::Installer ${SIZE_MB} MB exceeds ${CEILING} MB budget (DSK-03)"
    exit 1
fi

echo "OK: installer size within budget"
