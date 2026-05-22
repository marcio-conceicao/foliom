// CM6 extension array in the correct order.
// Per 03-RESEARCH §1: Prec.highest boundary keymap MUST be first so it
// intercepts Enter/Tab/Backspace/Arrows before CM6 defaultKeymap consumes them.
//
// Extension order:
// 1. Prec.highest boundary keymap  — intercepts boundary keys
// 2. history()                     — per-instance undo stack (EDT-10)
// 3. historyKeymap                 — Ctrl+Z / Ctrl+Shift+Z inside edit mode
// 4. autocompletion                — [[link]] and #tag completion (EDT-09)
// 5. markdown()                    — syntax highlighting (adds markdownKeymap with Prec.high)
// 6. defaultKeymap                 — lowest precedence fallback
// 7. drawSelection + lineWrapping  — visual affordances

import { Prec } from '@codemirror/state';
import { EditorView, keymap, drawSelection } from '@codemirror/view';
import { history, historyKeymap, defaultKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { autocompletion } from '@codemirror/autocomplete';
import type { Extension } from '@codemirror/state';
import type { BlockEditorCallbacks } from './boundary';

/**
 * Returns the CM6 extension array for the block editor.
 * Pass callbacks for boundary key handling and autocomplete.
 */
export function blockEditorExtensions(cb: BlockEditorCallbacks): Extension[] {
  return [
    // 1. Boundary keys at highest precedence — MUST come before markdown() and defaultKeymap.
    // Without Prec.highest, CM6's defaultKeymap consumes Backspace/Enter/Tab first
    // and our tree-op handlers never see the events. (03-RESEARCH §1, A8 mitigation)
    Prec.highest(
      keymap.of([
        { key: 'Enter', run: (v) => cb.onBoundary('Enter', v) },
        { key: 'Shift-Enter', run: (v) => cb.onBoundary('ShiftEnter', v) },
        { key: 'Tab', run: (v) => cb.onBoundary('Tab', v) },
        { key: 'Shift-Tab', run: (v) => cb.onBoundary('ShiftTab', v) },
        { key: 'Backspace', run: (v) => cb.onBoundary('Backspace', v) },
        { key: 'ArrowUp', run: (v) => cb.onBoundary('ArrowUp', v) },
        { key: 'ArrowDown', run: (v) => cb.onBoundary('ArrowDown', v) },
      ]),
    ),

    // 2+3. Per-instance history (EDT-10, §4).
    // history() lives in EditorState — view.destroy() drops it → no cross-block undo.
    // Per 03-RESEARCH §4: history() per-instance — each mount gets a fresh undo stack.
    history(),
    keymap.of(historyKeymap),

    // 4. Autocomplete (EDT-09, D-30-06). Plan 03-04 stubs completions with async () => null.
    // Real [[link]] and #tag completions wire in plan 03-06.
    autocompletion({ override: [cb.completions] }),

    // 5. Markdown grammar (syntax highlighting + GFM tables).
    // Note: markdown() internally adds markdownKeymap with Prec.high for Enter continuation.
    // Our Prec.highest boundary keymap runs first, so our Enter handler wins.
    // A2 verified: markdownKeymap only has Enter-related bindings, no Tab.
    markdown(),

    // 6. Default keymap — handles all other keys not intercepted above.
    keymap.of(defaultKeymap),

    // Visual affordances
    drawSelection(),
    EditorView.lineWrapping,
  ];
}
