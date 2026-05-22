// Tree-op log store.
// Per D-30-05: a separate frontend transaction log records each tree op as
// one undoable step. Ctrl+Z outside edit mode pops the last tree-op.
//
// Key properties (03-RESEARCH §Code Examples):
// - 200-entry FIFO cap (T-03-13 mitigation — prevents unbounded memory growth)
// - In-memory only — lost on page reload (per D-30-05 recommendation)
// - Distinct from CM6 history (which is per-block EditorView instance)
//
// TreeOp variants (D-30-05 + D-30-08):
//   Indent    — Tab pressed; prev depth stored for undo (PATCH /api/blocks/:id/structure)
//   Outdent   — Shift+Tab pressed; prev depth stored for undo
//   Merge     — Backspace-at-start of non-empty block (EDT-06)
//   Split     — Enter mid-text creates sibling (EDT-04)
//   Move      — Alt+Shift+Arrow block reorder
//   Delete    — Backspace on empty block (D-30-08)

import { writable, get } from 'svelte/store';

/**
 * Minimal snapshot for restoring a deleted block's position on undo.
 */
export interface BlockSnapshot {
  raw: string;
  depth: number;
  parentId: number | null;
  ord: number;
}

/**
 * Discriminated union of tree-level operations that can be undone.
 * Each variant stores exactly the information needed for its inverse op.
 */
export type TreeOp =
  | { kind: 'Indent'; blockId: number; prevDepth: number }
  | { kind: 'Outdent'; blockId: number; prevDepth: number }
  | { kind: 'Merge'; blockId: number; mergedIntoId: number; originalRaw: string }
  | { kind: 'Split'; blockId: number; atOffset: number; newBlockId: number }
  | { kind: 'Move'; blockId: number; prevParentId: number | null; prevOrd: number }
  | { kind: 'Delete'; blockId: number; snapshot: BlockSnapshot };

const CAP = 200;

function createTreeOpLog() {
  const { subscribe, update } = writable<TreeOp[]>([]);

  return {
    subscribe,
    /**
     * Push a new tree op onto the log.
     * If the log exceeds 200 entries, the oldest (first) entry is dropped (FIFO).
     */
    push(op: TreeOp): void {
      update((log) => {
        const next = [...log, op];
        return next.length > CAP ? next.slice(next.length - CAP) : next;
      });
    },
    /**
     * Pop the most-recent tree op and return it.
     * Returns undefined if the log is empty.
     */
    pop(): TreeOp | undefined {
      let out: TreeOp | undefined;
      update((log) => {
        if (log.length === 0) return log;
        out = log[log.length - 1];
        return log.slice(0, -1);
      });
      return out;
    },
    /** Clear the entire log. */
    clear(): void {
      update(() => []);
    },
  };
}

export const treeOpLog = createTreeOpLog();
