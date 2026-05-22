// Tests for treeOpLog store.
// Verifies: push/pop/clear semantics, 200-entry FIFO cap, typed TreeOp variants.
// Per 03-RESEARCH §Code Examples (Tree-Op Log Store) and D-30-05.

import { get } from 'svelte/store';
import { expect, test, describe, beforeEach } from 'vitest';
import { treeOpLog, type TreeOp } from '../treeOpLog';

beforeEach(() => {
  treeOpLog.clear();
});

describe('treeOpLog — basic operations', () => {
  test('starts empty', () => {
    expect(get(treeOpLog)).toHaveLength(0);
  });

  test('push adds an op', () => {
    const op: TreeOp = { kind: 'Indent', blockId: 1, prevDepth: 0 };
    treeOpLog.push(op);
    expect(get(treeOpLog)).toHaveLength(1);
    expect(get(treeOpLog)[0]).toEqual(op);
  });

  test('pop returns the last op and removes it', () => {
    treeOpLog.push({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    treeOpLog.push({ kind: 'Outdent', blockId: 2, prevDepth: 1 });
    const popped = treeOpLog.pop();
    expect(popped).toEqual({ kind: 'Outdent', blockId: 2, prevDepth: 1 });
    expect(get(treeOpLog)).toHaveLength(1);
  });

  test('pop on empty log returns undefined', () => {
    const result = treeOpLog.pop();
    expect(result).toBeUndefined();
    expect(get(treeOpLog)).toHaveLength(0);
  });

  test('clear empties the store', () => {
    treeOpLog.push({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    treeOpLog.push({ kind: 'Outdent', blockId: 2, prevDepth: 1 });
    treeOpLog.clear();
    expect(get(treeOpLog)).toHaveLength(0);
  });
});

describe('treeOpLog — FIFO 200-entry cap (T-03-13 mitigation)', () => {
  test('adding exactly 200 ops stays at 200', () => {
    for (let i = 0; i < 200; i++) {
      treeOpLog.push({ kind: 'Indent', blockId: i, prevDepth: 0 });
    }
    expect(get(treeOpLog)).toHaveLength(200);
  });

  test('adding 201 ops drops the oldest (FIFO)', () => {
    for (let i = 0; i < 201; i++) {
      treeOpLog.push({ kind: 'Indent', blockId: i, prevDepth: 0 });
    }
    const log = get(treeOpLog);
    expect(log).toHaveLength(200);
    // First entry is the second op (blockId=1), oldest (blockId=0) dropped
    expect(log[0]).toEqual({ kind: 'Indent', blockId: 1, prevDepth: 0 });
    // Last entry is the newest (blockId=200)
    expect(log[199]).toEqual({ kind: 'Indent', blockId: 200, prevDepth: 0 });
  });

  test('adding 300 ops drops 100 oldest', () => {
    for (let i = 0; i < 300; i++) {
      treeOpLog.push({ kind: 'Outdent', blockId: i, prevDepth: 2 });
    }
    const log = get(treeOpLog);
    expect(log).toHaveLength(200);
    expect(log[0]).toEqual({ kind: 'Outdent', blockId: 100, prevDepth: 2 });
    expect(log[199]).toEqual({ kind: 'Outdent', blockId: 299, prevDepth: 2 });
  });
});

describe('treeOpLog — TreeOp variants', () => {
  test('Indent op', () => {
    const op: TreeOp = { kind: 'Indent', blockId: 5, prevDepth: 1 };
    treeOpLog.push(op);
    expect(treeOpLog.pop()).toEqual(op);
  });

  test('Outdent op', () => {
    const op: TreeOp = { kind: 'Outdent', blockId: 5, prevDepth: 2 };
    treeOpLog.push(op);
    expect(treeOpLog.pop()).toEqual(op);
  });

  test('Merge op', () => {
    const op: TreeOp = { kind: 'Merge', blockId: 5, mergedIntoId: 4, originalRaw: '- text' };
    treeOpLog.push(op);
    expect(treeOpLog.pop()).toEqual(op);
  });

  test('Split op', () => {
    const op: TreeOp = { kind: 'Split', blockId: 5, atOffset: 10, newBlockId: 6 };
    treeOpLog.push(op);
    expect(treeOpLog.pop()).toEqual(op);
  });

  test('Move op', () => {
    const op: TreeOp = { kind: 'Move', blockId: 5, prevParentId: 3, prevOrd: 1 };
    treeOpLog.push(op);
    expect(treeOpLog.pop()).toEqual(op);
  });

  test('Delete op with snapshot', () => {
    const op: TreeOp = {
      kind: 'Delete',
      blockId: 5,
      snapshot: { raw: '- hello', depth: 0, parentId: null, ord: 0 },
    };
    treeOpLog.push(op);
    expect(treeOpLog.pop()).toEqual(op);
  });
});
