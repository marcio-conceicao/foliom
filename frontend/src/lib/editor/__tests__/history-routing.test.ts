// Tests for window-level Ctrl+Z routing (D-30-05).
// When document.activeElement is inside a .block.editing → CM6 handles it (do nothing).
// When activeElement is body or outside editing block → treeOpLog.pop() is called.

import { get } from 'svelte/store';
import { expect, test, vi, describe, beforeEach, afterEach } from 'vitest';
import { treeOpLog, type TreeOp } from '../../stores/treeOpLog';
import { bindHistoryRouting } from '../history-routing';

beforeEach(() => {
  treeOpLog.clear();
  document.body.innerHTML = '';
});

let disposer: (() => void) | null = null;
afterEach(() => {
  if (disposer) { disposer(); disposer = null; }
  treeOpLog.clear();
  document.body.innerHTML = '';
});

function dispatchCtrlZ(shiftKey = false): void {
  const event = new KeyboardEvent('keydown', {
    key: 'z',
    ctrlKey: true,
    shiftKey,
    bubbles: true,
    cancelable: true,
  });
  window.dispatchEvent(event);
}

describe('bindHistoryRouting — returns disposer', () => {
  test('returns a function disposer', () => {
    disposer = bindHistoryRouting();
    expect(typeof disposer).toBe('function');
  });

  test('disposer removes the listener (Ctrl+Z no longer triggers log pop)', () => {
    treeOpLog.push({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    const d = bindHistoryRouting();
    d(); // dispose immediately
    dispatchCtrlZ();
    // Should still have the op (listener was removed)
    expect(get(treeOpLog)).toHaveLength(1);
  });
});

describe('bindHistoryRouting — routing logic', () => {
  test('Ctrl+Z with activeElement=body pops treeOpLog', () => {
    treeOpLog.push({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    treeOpLog.push({ kind: 'Outdent', blockId: 2, prevDepth: 1 });
    disposer = bindHistoryRouting();

    // activeElement defaults to document.body in happy-dom
    expect(document.activeElement).toBe(document.body);
    dispatchCtrlZ();

    // Last op should be popped
    expect(get(treeOpLog)).toHaveLength(1);
    expect(get(treeOpLog)[0]).toEqual({ kind: 'Indent', blockId: 1, prevDepth: 0 });
  });

  test('Ctrl+Z inside .block.editing does NOT pop treeOpLog (CM6 handles it)', () => {
    treeOpLog.push({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    disposer = bindHistoryRouting();

    // Create a mock editing block in the DOM
    const block = document.createElement('div');
    block.classList.add('block', 'editing');
    const cmContent = document.createElement('div');
    cmContent.classList.add('cm-content');
    block.appendChild(cmContent);
    document.body.appendChild(block);

    // Focus the cm-content element to simulate CM6 active
    cmContent.focus();
    // Manually set activeElement via focus
    Object.defineProperty(document, 'activeElement', {
      get: () => cmContent,
      configurable: true,
    });

    dispatchCtrlZ();

    // treeOpLog should NOT have been popped
    expect(get(treeOpLog)).toHaveLength(1);

    // Restore activeElement
    Object.defineProperty(document, 'activeElement', {
      get: () => document.body,
      configurable: true,
    });
  });

  test('Ctrl+Z on empty log is safe (no throw)', () => {
    disposer = bindHistoryRouting();
    expect(() => dispatchCtrlZ()).not.toThrow();
    expect(get(treeOpLog)).toHaveLength(0);
  });

  test('Ctrl+Shift+Z pops treeOpLog when outside editing (redo placeholder)', () => {
    // In this plan, Ctrl+Shift+Z outside edit mode also routes to treeOpLog
    // (full redo implementation deferred to 03-05; here it's the same pop path).
    treeOpLog.push({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    disposer = bindHistoryRouting();
    dispatchCtrlZ(true); // Shift+Z
    // Behavior: same pop path as Ctrl+Z (plan 03-04 skeleton; real redo in 03-05)
    expect(get(treeOpLog)).toHaveLength(0);
  });
});
