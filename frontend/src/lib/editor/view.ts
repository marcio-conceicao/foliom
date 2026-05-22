// BlockEditor class — mount/unmount discipline + readDocSafe IME guard.
// Per 03-RESEARCH §Code Examples and §1 (single CM6 instance per focused block).
//
// Key invariants:
// 1. At most one EditorView is mounted at a time per BlockEditor instance.
//    mount() throws 'BlockEditor double-mount' if already mounted.
// 2. readDocSafe() returns null when view.composing === true (IME guard, EDT-13, T-03-10).
//    Callers MUST NOT save when null is returned.
// 3. unmount() reads the doc BEFORE destroying the view (state is dropped after destroy()).
//
// Per 03-RESEARCH §4: history() lives in EditorState; view.destroy() drops it →
// no cross-block undo. This is intentional — each mount has a fresh history.

import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { blockEditorExtensions } from './extensions';
import type { BlockEditorCallbacks } from './boundary';

export class BlockEditor {
  view: EditorView | null = null;

  /**
   * Mount a CM6 EditorView into `parent` with `initialRaw` as the document.
   * Throws 'BlockEditor double-mount' if already mounted.
   */
  mount(parent: HTMLElement, initialRaw: string, callbacks: BlockEditorCallbacks): void {
    if (this.view) throw new Error('BlockEditor double-mount');
    const state = EditorState.create({
      doc: initialRaw,
      extensions: blockEditorExtensions(callbacks),
    });
    this.view = new EditorView({ state, parent });
    this.view.focus();
  }

  /**
   * IME-safe document read.
   * Returns null if IME is composing (view.composing === true) — caller must defer.
   * Returns null if no view is mounted.
   */
  readDocSafe(): string | null {
    if (!this.view) return null;
    if (this.view.composing) return null;
    return this.view.state.doc.toString();
  }

  /**
   * Unmount the EditorView. Reads the doc (IME-safe) before destroying.
   * Returns the doc string (or null if IME was composing or no view).
   * Safe to call multiple times — subsequent calls are no-ops returning null.
   */
  unmount(): string | null {
    if (!this.view) return null;
    // IMPORTANT: read BEFORE destroy — state is dropped after view.destroy().
    const doc = this.readDocSafe();
    this.view.destroy();
    this.view = null;
    return doc;
  }
}

/**
 * IME-safe save gate.
 * Returns 'saved' if the doc can be read (caller should proceed to PUT /api/blocks/:id).
 * Returns 'skipped-due-to-ime' if IME composition is in progress (T-03-10 mitigation).
 * Returns 'no-editor' if no view is mounted.
 *
 * This wrapper does NOT call onSave — callers are responsible for reading readDocSafe()
 * and sending the PUT request. The function is a gate, not an orchestrator.
 */
export function trySaveBlock(
  editor: BlockEditor,
): 'saved' | 'skipped-due-to-ime' | 'no-editor' {
  if (!editor.view) return 'no-editor';
  const doc = editor.readDocSafe();
  if (doc === null) return 'skipped-due-to-ime';
  return 'saved';
}
