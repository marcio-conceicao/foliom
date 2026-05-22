// IME guard tests (EDT-13, T-03-10).
// Tests that view.composing gate prevents save during IME composition.
//
// A5 fallback: happy-dom's CompositionEvent dispatch MAY NOT flip view.composing
// because CM6's internal compositionstart listener may not fire via happy-dom's
// synthetic events. We test both approaches:
// 1. Dispatch CompositionEvent and check if view.composing toggles (canonical).
// 2. If not, use Object.defineProperty monkey-patch to assert the guard contract.
// Both test paths are intentional and documented per the plan.

import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { expect, test, vi, describe, afterEach } from 'vitest';
import { blockEditorExtensions } from '../extensions';
import { BlockEditor, trySaveBlock } from '../view';

afterEach(() => {
  document.body.innerHTML = '';
});

function mountEditor(raw = 'foo') {
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const onSave = vi.fn();
  const state = EditorState.create({
    doc: raw,
    extensions: blockEditorExtensions({
      onBoundary: vi.fn().mockReturnValue(false),
      onSave,
      completions: async () => null,
    }),
  });
  const view = new EditorView({ state, parent });
  return { view, parent, onSave };
}

describe('IME guard — view.composing property', () => {
  test('A1: view.composing exists and is a boolean', () => {
    // This is the fail-fast assertion per plan spec.
    // If this fails, view.composing was renamed in CM6 — check index.d.ts.
    const { view } = mountEditor();
    expect('composing' in view).toBe(true);
    expect(typeof view.composing).toBe('boolean');
    view.destroy();
  });

  test('view.composing is false by default', () => {
    const { view } = mountEditor();
    expect(view.composing).toBe(false);
    view.destroy();
  });
});

describe('IME guard — trySaveBlock with BlockEditor', () => {
  test('trySaveBlock returns saved when not composing', () => {
    const parent = document.createElement('div');
    document.body.appendChild(parent);
    const editor = new BlockEditor();
    editor.mount(parent, 'hello', {
      onBoundary: vi.fn().mockReturnValue(false),
      onSave: vi.fn(),
      completions: async () => null,
    });
    expect(trySaveBlock(editor)).toBe('saved');
    editor.unmount();
  });

  test('A5 canonical: CompositionEvent dispatch — if view.composing toggles, save is blocked', () => {
    const { view, parent, onSave } = mountEditor('foo');

    // Dispatch compositionstart to simulate IME activation
    const contentEl = parent.querySelector('.cm-content')!;
    contentEl.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    contentEl.dispatchEvent(new CompositionEvent('compositionupdate', { data: '~', bubbles: true }));

    const composingAfterStart = view.composing;
    // We intentionally do NOT assert composingAfterStart === true here because
    // happy-dom may not flip it (A5 uncertainty). Instead we test both branches:

    if (composingAfterStart) {
      // happy-dom DID flip view.composing — test the canonical path
      const editor = new BlockEditor();
      // Manually set editor's internal view to the already-mounted view
      // (for testing the guard in isolation)
      editor.view = view;
      const result = trySaveBlock(editor);
      expect(result).toBe('skipped-due-to-ime');
      // Detach so unmount doesn't crash
      editor.view = null;
    } else {
      // A5 fallback: happy-dom does NOT flip view.composing.
      // Test the guard contract via monkey-patch.
      // This is documented per plan spec as an explicit fallback.
      const originalDescriptor = Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(view),
        'composing',
      );
      Object.defineProperty(view, 'composing', { value: true, configurable: true });

      const editor = new BlockEditor();
      editor.view = view;
      const result = trySaveBlock(editor);
      expect(result).toBe('skipped-due-to-ime');

      // Restore
      editor.view = null;
      if (originalDescriptor) {
        Object.defineProperty(view, 'composing', originalDescriptor);
      } else {
        // Remove the own property so prototype accessor is active again
        delete (view as unknown as Record<string, unknown>).composing;
      }
    }

    view.destroy();
  });

  test('A5 canonical: after compositionend, trySaveBlock returns saved', () => {
    const { view, parent } = mountEditor('foo');
    const contentEl = parent.querySelector('.cm-content')!;

    contentEl.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    contentEl.dispatchEvent(new CompositionEvent('compositionupdate', { data: '~', bubbles: true }));
    contentEl.dispatchEvent(new CompositionEvent('compositionend', { data: 'ã', bubbles: true }));

    // After compositionend, composing should be false
    expect(view.composing).toBe(false);

    const editor = new BlockEditor();
    editor.view = view;
    const result = trySaveBlock(editor);
    expect(result).toBe('saved');
    editor.view = null;
    view.destroy();
  });

  test('A5 fallback: monkey-patched composing=true → trySaveBlock returns skipped-due-to-ime', () => {
    // Explicit test of the guard contract independent of happy-dom CompositionEvent behavior.
    // This is the documented fallback for A5.
    const parent = document.createElement('div');
    document.body.appendChild(parent);
    const editor = new BlockEditor();
    editor.mount(parent, 'test', {
      onBoundary: vi.fn().mockReturnValue(false),
      onSave: vi.fn(),
      completions: async () => null,
    });

    // Monkey-patch the instance property
    Object.defineProperty(editor.view!, 'composing', { value: true, configurable: true });
    expect(trySaveBlock(editor)).toBe('skipped-due-to-ime');

    // Restore and verify it saves now
    delete (editor.view! as unknown as Record<string, unknown>).composing;
    expect(trySaveBlock(editor)).toBe('saved');

    editor.unmount();
  });
});
