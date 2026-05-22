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
// treeOpLog and invokes a placeholder inverse op (console log in plan 03-04;
// real inverse application lands in plan 03-05).
//
// Returns a disposer for cleanup (HMR / test teardown).

import { treeOpLog } from '../stores/treeOpLog';

function isInsideEditingBlock(el: Element | null): boolean {
  if (!el) return false;
  return el.closest('.block.editing') !== null;
}

/**
 * Bind the global Ctrl+Z / Ctrl+Shift+Z listener.
 * Call once from App.svelte's $effect.
 * Returns a disposer that removes the listener.
 */
export function bindHistoryRouting(): () => void {
  function handler(e: KeyboardEvent): void {
    const isMod = e.ctrlKey || e.metaKey;
    const isZ = e.key.toLowerCase() === 'z';
    if (!isMod || !isZ) return;

    // If active element is inside a CM6-mounted .block.editing, let CM6 handle it.
    if (isInsideEditingBlock(document.activeElement as Element | null)) {
      return;
    }

    // Outside edit mode: pop the tree-op log and invoke placeholder inverse.
    e.preventDefault();
    const op = treeOpLog.pop();
    if (op) {
      // Plan 03-04 skeleton: real inverse application lands in plan 03-05.
      // The log is correct; the inverse op is a placeholder until 03-05 wires it.
      console.debug('[treeOpLog inverse]', op);
    }
  }

  window.addEventListener('keydown', handler);
  return () => window.removeEventListener('keydown', handler);
}
