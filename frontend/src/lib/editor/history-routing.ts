// Window-level Ctrl+Z / Ctrl+Shift+Z routing (D-30-05).
//
// Rule: "Ctrl+Z while focused in CM6 always uses CM6 history;
//        Ctrl+Z while focus is on a read-only block (or document body)
//        uses tree-op log." — from D-30-05 made concrete in 03-RESEARCH §4.
//
// When document.activeElement is inside a .block.editing element,
// CM6's own historyKeymap (bound with Prec.highest via blockEditorExtensions)
// handles Ctrl+Z / Ctrl+Shift+Z — we do nothing.
//
// When activeElement is outside any .block.editing, Ctrl+Z pops the
// treeOpLog and invokes applyInverse (plan 03-05).
//
// Returns a disposer for cleanup (HMR / test teardown).

import { treeOpLog } from '../stores/treeOpLog';
import type { TreeOp } from '../stores/treeOpLog';

function isInsideEditingBlock(el: Element | null): boolean {
  if (!el) return false;
  return el.closest('.block.editing') !== null;
}

/**
 * Apply the inverse of a TreeOp against the server.
 *
 * Per plan 03-05 spec:
 *   - Indent / Outdent → PATCH /api/blocks/:id/structure with depth = prevDepth
 *   - Delete           → POST  /api/blocks to restore the snapshot
 *   - Move             → PATCH /api/blocks/:id/structure with parent_id + ord
 *   - Merge / Split    → complex; deferred to plan 03-06 (stub logs + returns)
 *
 * @param op          The TreeOp to invert.
 * @param prevHash    The current file hash (from pageDetail.fileHash).
 * @param pageId      Page ID — required for Delete (POST /api/blocks needs it).
 * @param onConflict  Called with the op if server returns 409 so caller can
 *                    re-push to treeOpLog and surface the stale banner.
 */
export async function applyInverse(
  op: TreeOp,
  prevHash: string,
  pageId?: number,
  onConflict?: (op: TreeOp) => void,
): Promise<void> {
  try {
    switch (op.kind) {
      case 'Indent':
      case 'Outdent': {
        // Restore previous depth via PATCH /structure
        const res = await fetch(`/api/blocks/${op.blockId}/structure`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
          body: JSON.stringify({ op: 'move', prevHash, depth: op.prevDepth }),
        });
        if (res.status === 409) {
          onConflict?.(op);
          return;
        }
        if (!res.ok) {
          console.error(`[applyInverse] PATCH failed: ${res.status}`, op);
        }
        break;
      }

      case 'Delete': {
        // Restore the deleted block via POST /api/blocks
        const { snapshot } = op;
        const res = await fetch('/api/blocks', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
          body: JSON.stringify({
            pageId: pageId ?? 0,
            parentId: snapshot.parentId,
            ord: snapshot.ord,
            depth: snapshot.depth,
            raw: snapshot.raw,
            prevHash,
          }),
        });
        if (res.status === 409) {
          onConflict?.(op);
          return;
        }
        if (!res.ok) {
          console.error(`[applyInverse] POST failed: ${res.status}`, op);
        }
        break;
      }

      case 'Move': {
        // Restore previous position via PATCH /structure
        const res = await fetch(`/api/blocks/${op.blockId}/structure`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
          body: JSON.stringify({
            op: 'move',
            prevHash,
            parentId: op.prevParentId,
            ord: op.prevOrd,
          }),
        });
        if (res.status === 409) {
          onConflict?.(op);
          return;
        }
        if (!res.ok) {
          console.error(`[applyInverse] PATCH move failed: ${res.status}`, op);
        }
        break;
      }

      case 'Merge':
      case 'Split': {
        // Complex inverse — deferred to plan 03-06.
        // Split inverse: deleteBlock(newBlockId) + putBlock(originalId, rawBeforeSplit)
        // Merge inverse: postBlock(recreate merged block) + putBlock(predecessor, pre-merge raw)
        // For now, log as unsupported — the op details are preserved but not executed.
        console.debug('[applyInverse] Merge/Split inverse not yet implemented', op);
        break;
      }

      default:
        // Exhaustive check — TypeScript will warn if a variant is missing.
        console.warn('[applyInverse] unknown op kind', op);
    }
  } catch (e) {
    console.error('[applyInverse] network error', e, op);
  }
}

/**
 * Bind the global Ctrl+Z / Ctrl+Shift+Z listener.
 * Call once from App.svelte's $effect.
 * Returns a disposer that removes the listener.
 *
 * @param getPageHash  Optional callback to get the current page file hash.
 * @param getPageId    Optional callback to get the current page ID.
 * @param onConflict   Optional callback to surface stale-conflict banner.
 */
export function bindHistoryRouting(
  getPageHash?: () => string,
  getPageId?: () => number | undefined,
  onConflict?: (op: TreeOp) => void,
): () => void {
  function handler(e: KeyboardEvent): void {
    const isMod = e.ctrlKey || e.metaKey;
    const isZ = e.key.toLowerCase() === 'z';
    if (!isMod || !isZ) return;

    // If active element is inside a CM6-mounted .block.editing, let CM6 handle it.
    if (isInsideEditingBlock(document.activeElement as Element | null)) {
      return;
    }

    // Outside edit mode: pop the tree-op log and invoke inverse.
    e.preventDefault();
    const op = treeOpLog.pop();
    if (op) {
      const prevHash = getPageHash?.() ?? '';
      const pageId = getPageId?.();

      void applyInverse(op, prevHash, pageId, (restoredOp) => {
        // On 409: restore the op to the log and surface the banner.
        treeOpLog.push(restoredOp);
        onConflict?.(restoredOp);
      });
    }
  }

  window.addEventListener('keydown', handler);
  return () => window.removeEventListener('keydown', handler);
}
