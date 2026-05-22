// frontend/src/lib/keys.ts
//
// Global keymap registrar (SCH-03). A single `keydown` listener on
// `window` owns the application-level shortcuts so individual
// components don't have to fight each other for focus or event order.
//
// Current bindings:
//   - Ctrl+K / Cmd+K → toggle searchPalette open/closed (works even when
//     focus is inside an <input>/<textarea>/[contenteditable] — the
//     modifier overrides the input gate by design so users always have a
//     way out).
//   - Esc → close the palette IF open. Suppressed when focus is inside
//     an input/textarea/contenteditable so the keystroke can clear the
//     input's value via native browser behavior instead.
//
// Returns a disposer for HMR / test cleanup.

import { get } from 'svelte/store';
import { searchPalette } from './stores';

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (target.isContentEditable) return true;
  return false;
}

export function bindGlobalShortcuts(): () => void {
  function handler(e: KeyboardEvent): void {
    const isCmdK = (e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'k';
    if (isCmdK) {
      // The modifier explicitly overrides the input gate — users must
      // always be able to summon the palette regardless of focus.
      e.preventDefault();
      const cur = get(searchPalette);
      searchPalette.set({ open: !cur.open, query: '' });
      return;
    }

    if (e.key === 'Escape') {
      // Inside an input/textarea/contenteditable we let Esc fall through
      // so the native "clear input" behavior wins. Outside those, Esc
      // closes the palette if it's open.
      if (isEditableTarget(e.target)) return;
      const cur = get(searchPalette);
      if (cur.open) {
        e.preventDefault();
        searchPalette.set({ open: false, query: '' });
      }
    }
  }

  window.addEventListener('keydown', handler);
  return () => window.removeEventListener('keydown', handler);
}
