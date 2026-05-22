// Boundary key enum and handler type definitions.
// These represent the keys intercepted by the Prec.highest keymap in extensions.ts.
// Per 03-RESEARCH §1 boundary table and D-30-08.

import type { EditorView } from '@codemirror/view';

/**
 * Keys that trigger block-level operations instead of CM6 character insertion.
 * Each is mapped by a Prec.highest keymap entry in extensions.ts.
 */
export type BoundaryKey =
  | 'Enter'       // Save current block + create sibling (EDT-04)
  | 'ShiftEnter'  // Insert newline within block (returns false → CM6 default)
  | 'Tab'         // Indent block (EDT-05)
  | 'ShiftTab'    // Outdent block (EDT-05)
  | 'Backspace'   // Merge with prev (non-empty) or Delete block (empty) (EDT-06, D-30-08)
  | 'ArrowUp'     // Navigate to prev block when cursor at first line (EDT-07)
  | 'ArrowDown';  // Navigate to next block when cursor at last line (EDT-07)

/**
 * Callbacks passed to the block editor on mount.
 * onBoundary returns true if the event was handled (prevents CM6 default).
 * onSave is called with the final doc string after unmount.
 * completions is the CM6 autocomplete source.
 * onPaste (optional) — called with clipboard text; return true if handled.
 *   If returns true, CM6's default paste is suppressed.
 */
export interface BlockEditorCallbacks {
  onBoundary: (key: BoundaryKey, view: EditorView) => boolean;
  onSave: (raw: string) => void;
  completions: (ctx: import('@codemirror/autocomplete').CompletionContext) => Promise<import('@codemirror/autocomplete').CompletionResult | null>;
  onPaste?: (clipboardText: string, view: EditorView) => boolean | Promise<boolean>;
}
