// Tests for BlockEditor class: mount/unmount discipline + history per-instance.
// These test the view.ts module which provides BlockEditor and trySaveBlock.

import { EditorView } from '@codemirror/view';
import { expect, test, vi, describe, afterEach } from 'vitest';
import { BlockEditor, trySaveBlock } from '../view';

// Cleanup helper
let editors: BlockEditor[] = [];
afterEach(() => {
  for (const e of editors) {
    try { e.unmount(); } catch { /* already unmounted */ }
  }
  editors = [];
  document.body.innerHTML = '';
});

function makeParent(): HTMLElement {
  const el = document.createElement('div');
  document.body.appendChild(el);
  return el;
}

function makeEditor(raw = '- hello'): { editor: BlockEditor; parent: HTMLElement } {
  const parent = makeParent();
  const editor = new BlockEditor();
  editors.push(editor);
  const onBoundary = vi.fn().mockReturnValue(false);
  const onSave = vi.fn();
  editor.mount(parent, raw, {
    onBoundary,
    onSave,
    completions: async () => null,
  });
  return { editor, parent };
}

describe('BlockEditor A1 — view.composing exists', () => {
  test('view.composing is a boolean property on the mounted EditorView', () => {
    const { editor } = makeEditor();
    expect(editor.view).not.toBeNull();
    // A1: fail-fast if CM6 renames the property
    expect('composing' in editor.view!).toBe(true);
    expect(typeof editor.view!.composing).toBe('boolean');
  });
});

describe('BlockEditor — mount/unmount discipline', () => {
  test('mount creates a CM6 view with cm-content element', () => {
    const { parent } = makeEditor();
    expect(parent.querySelector('.cm-content')).not.toBeNull();
  });

  test('unmount destroys the view and returns the doc string', () => {
    const { editor } = makeEditor('- hello');
    const doc = editor.unmount();
    expect(doc).toBe('- hello');
    expect(editor.view).toBeNull();
  });

  test('double-mount throws BlockEditor double-mount error', () => {
    const parent = makeParent();
    const editor = new BlockEditor();
    editors.push(editor);
    editor.mount(parent, '- first', {
      onBoundary: vi.fn().mockReturnValue(false),
      onSave: vi.fn(),
      completions: async () => null,
    });
    expect(() => {
      editor.mount(parent, '- second', {
        onBoundary: vi.fn().mockReturnValue(false),
        onSave: vi.fn(),
        completions: async () => null,
      });
    }).toThrow('BlockEditor double-mount');
  });

  test('unmount on already-unmounted editor returns null and does not throw', () => {
    const { editor } = makeEditor();
    editor.unmount();
    expect(() => editor.unmount()).not.toThrow();
    expect(editor.unmount()).toBeNull();
  });

  test('readDocSafe returns null when view is null', () => {
    const editor = new BlockEditor();
    editors.push(editor);
    expect(editor.readDocSafe()).toBeNull();
  });
});

describe('BlockEditor — per-instance history isolation', () => {
  test('undo after remount does not reach previous instance history', async () => {
    const parent = makeParent();
    const editor = new BlockEditor();
    editors.push(editor);
    // Mount with doc "foo"
    editor.mount(parent, 'foo', {
      onBoundary: vi.fn().mockReturnValue(false),
      onSave: vi.fn(),
      completions: async () => null,
    });

    // Insert "bar" at position 3
    editor.view!.dispatch({
      changes: { from: 3, insert: 'bar' },
    });
    expect(editor.view!.state.doc.toString()).toBe('foobar');

    // Undo (Mod-z) — CM6 undo
    const undoCmd = editor.view!.dispatch.bind(editor.view!);
    // Use keyboard shortcut dispatch via the commands module
    const { undo } = await import('@codemirror/commands');
    const result = undo(editor.view!);
    expect(result).toBe(true);
    expect(editor.view!.state.doc.toString()).toBe('foo');

    // Destroy and remount with fresh doc "fresh"
    editor.unmount();
    const parent2 = makeParent();
    editor.mount(parent2, 'fresh', {
      onBoundary: vi.fn().mockReturnValue(false),
      onSave: vi.fn(),
      completions: async () => null,
    });
    // Ctrl+Z on the new instance — should have NO history (fresh state)
    const result2 = undo(editor.view!);
    // undo returns false when no history to undo
    expect(result2).toBe(false);
    expect(editor.view!.state.doc.toString()).toBe('fresh');
  });
});

describe('trySaveBlock', () => {
  test('returns no-editor when BlockEditor has no view', () => {
    const editor = new BlockEditor();
    editors.push(editor);
    expect(trySaveBlock(editor)).toBe('no-editor');
  });

  test('returns saved when view exists and not composing', () => {
    const { editor } = makeEditor('- hello');
    expect(trySaveBlock(editor)).toBe('saved');
  });
});
