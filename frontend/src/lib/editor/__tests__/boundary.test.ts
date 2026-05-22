// Boundary keymap tests.
// Tests that Prec.highest keymap intercepts boundary keys BEFORE CM6 defaults.
// Also verifies A2: markdown() does not ship a Tab binding.

import { EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { markdown } from '@codemirror/lang-markdown';
import { expect, test, vi, describe, afterEach } from 'vitest';
import { blockEditorExtensions } from '../extensions';
import { BlockEditor } from '../view';

afterEach(() => {
  document.body.innerHTML = '';
});

function mountWithBoundaryMock() {
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const onBoundary = vi.fn().mockReturnValue(true); // intercept all boundary keys
  const editor = new BlockEditor();
  editor.mount(parent, '- hello', {
    onBoundary,
    onSave: vi.fn(),
    completions: async () => null,
  });
  return { editor, parent, onBoundary };
}

function dispatchKey(view: EditorView, key: string, shiftKey = false): void {
  const contentEl = view.dom.querySelector('.cm-content') ?? view.dom;
  contentEl.dispatchEvent(
    new KeyboardEvent('keydown', {
      key,
      shiftKey,
      bubbles: true,
      cancelable: true,
    }),
  );
}

describe('A2 — markdown() has no Tab binding', () => {
  test('markdown() extension ships no Tab keybinding', () => {
    const state = EditorState.create({
      doc: 'test',
      extensions: [markdown()],
    });
    const view = new EditorView({ state, parent: document.createElement('div') });
    // Inspect all keymaps for Tab entries
    const keymaps = view.state.facet(keymap as Parameters<typeof view.state.facet>[0]);
    const allBindings = (keymaps as Array<Array<{ key?: string }>>[]).flat().flat();
    const tabBindings = allBindings.filter((b) => b && b.key === 'Tab');
    expect(tabBindings).toHaveLength(0);
    view.destroy();
  });
});

describe('Boundary keymap — Prec.highest interception', () => {
  test('Enter dispatches to onBoundary before CM6 default', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    // Move cursor to position 0
    editor.view!.dispatch({ selection: { anchor: 0 } });
    dispatchKey(editor.view!, 'Enter');
    expect(onBoundary).toHaveBeenCalledWith('Enter', editor.view);
    editor.unmount();
  });

  test('Backspace at position 0 dispatches to onBoundary', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    editor.view!.dispatch({ selection: { anchor: 0 } });
    dispatchKey(editor.view!, 'Backspace');
    expect(onBoundary).toHaveBeenCalledWith('Backspace', editor.view);
    editor.unmount();
  });

  test('Tab dispatches to onBoundary', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    dispatchKey(editor.view!, 'Tab');
    expect(onBoundary).toHaveBeenCalledWith('Tab', editor.view);
    editor.unmount();
  });

  test('Shift+Tab dispatches to onBoundary', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    dispatchKey(editor.view!, 'Tab', true);
    expect(onBoundary).toHaveBeenCalledWith('ShiftTab', editor.view);
    editor.unmount();
  });

  test('ArrowUp dispatches to onBoundary', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    editor.view!.dispatch({ selection: { anchor: 0 } });
    dispatchKey(editor.view!, 'ArrowUp');
    expect(onBoundary).toHaveBeenCalledWith('ArrowUp', editor.view);
    editor.unmount();
  });

  test('ArrowDown dispatches to onBoundary', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    // Position cursor at end of doc for ArrowDown
    const docLen = editor.view!.state.doc.length;
    editor.view!.dispatch({ selection: { anchor: docLen } });
    dispatchKey(editor.view!, 'ArrowDown');
    expect(onBoundary).toHaveBeenCalledWith('ArrowDown', editor.view);
    editor.unmount();
  });
});

describe('Boundary keymap — boundary.ts BoundaryKey handler logic', () => {
  test('Backspace on empty block triggers Backspace boundary (D-30-08 Delete path)', () => {
    const parent = document.createElement('div');
    document.body.appendChild(parent);
    const onBoundary = vi.fn().mockReturnValue(true);
    const editor = new BlockEditor();
    // Empty doc
    editor.mount(parent, '', {
      onBoundary,
      onSave: vi.fn(),
      completions: async () => null,
    });
    editor.view!.dispatch({ selection: { anchor: 0 } });
    dispatchKey(editor.view!, 'Backspace');
    // Should have been called with Backspace
    expect(onBoundary).toHaveBeenCalledWith('Backspace', editor.view);
    editor.unmount();
  });

  test('Shift+Enter dispatches to onBoundary with ShiftEnter key', () => {
    const { editor, onBoundary } = mountWithBoundaryMock();
    dispatchKey(editor.view!, 'Enter', true);
    expect(onBoundary).toHaveBeenCalledWith('ShiftEnter', editor.view);
    editor.unmount();
  });
});
