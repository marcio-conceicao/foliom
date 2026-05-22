#!/usr/bin/env bash
# scripts/acpt05_inspect.sh
#
# Convenience script for ACPT-05 manual portability inspection.
#
# Usage:
#   bash scripts/acpt05_inspect.sh
#
# Environment variables:
#   ACPT05_OUT  — where to write the post-edit corpus (default: /tmp/foliom-acpt05)
#
# After the script completes:
#   1. Open $ACPT05_OUT as an Obsidian vault.
#   2. Open $ACPT05_OUT in VS Code.
#   3. Follow the checklist in:
#      .planning/phases/03-outliner-editor/ACPT-05-PORTABILITY.md
set -euo pipefail

OUT="${ACPT05_OUT:-/tmp/foliom-acpt05}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "[acpt05_inspect] Repo root: ${REPO_ROOT}"
echo "[acpt05_inspect] Output dir: ${OUT}"

# Clean previous output
rm -rf "${OUT}"
mkdir -p "${OUT}"

# Run the automated test with keep-tempdir enabled.
cd "${REPO_ROOT}"
ACPT05_KEEP_TEMPDIR=1 cargo test -p foliom-cli --test portability_acpt_05 -- --nocapture

echo ""
echo "======================================="
echo "[acpt05_inspect] Automated test PASSED."
echo "======================================="
echo ""
echo "Post-edit corpus available at: ${OUT}"
echo ""
echo "Next steps:"
echo "  1. Open ${OUT} as an Obsidian vault"
echo "  2. Open ${OUT} in VS Code"
echo "  3. Follow checklist: .planning/phases/03-outliner-editor/ACPT-05-PORTABILITY.md"
echo ""
echo "Pre-edit fixtures are at:"
echo "  crates/cli/tests/fixtures/acpt-05/before/"
echo "  crates/core/tests/fixtures/logseq-synthetic/"
