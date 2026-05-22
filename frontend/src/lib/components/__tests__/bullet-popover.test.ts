// Tests for BulletPopover + Block.svelte wiring + serialize.ts + treeOpLog inverse.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';
import { get } from 'svelte/store';
import { treeOpLog } from '../../stores/treeOpLog';
import type { Block } from '../../api';

// ─── helpers ──────────────────────────────────────────────────────────────────

function makeBlock(overrides: Partial<Block> = {}): Block {
  return {
    id: 1,
    depth: 0,
    raw: '- root block\n',
    properties: [],
    drawers: [],
    children: [],
    ...overrides,
  };
}

function makeBlockWithChildren(): Block {
  return {
    id: 1,
    depth: 0,
    raw: '- parent\n',
    properties: [],
    drawers: [],
    children: [
      {
        id: 2,
        depth: 1,
        raw: '\t- child1\n',
        properties: [],
        drawers: [],
        children: [],
      },
      {
        id: 3,
        depth: 1,
        raw: '\t- child2\n',
        properties: [],
        drawers: [],
        children: [
          {
            id: 4,
            depth: 2,
            raw: '\t\t- grandchild\n',
            properties: [],
            drawers: [],
            children: [],
          },
        ],
      },
    ],
  };
}

// ─── serialize.ts tests ───────────────────────────────────────────────────────

describe('serializeBlockTree', () => {
  let serializeBlockTree: (block: Block) => string;

  beforeEach(async () => {
    const mod = await import('../../editor/serialize');
    serializeBlockTree = mod.serializeBlockTree;
  });

  it('returns the raw of a leaf block', () => {
    const block = makeBlock({ raw: '- hello\n' });
    expect(serializeBlockTree(block)).toBe('- hello\n');
  });

  it('concatenates parent and children depth-first', () => {
    const block = makeBlockWithChildren();
    const result = serializeBlockTree(block);
    expect(result).toBe('- parent\n\t- child1\n\t- child2\n\t\t- grandchild\n');
  });

  it('round-trip: serializeBlockTree then detectBulletTree preserves item count', async () => {
    const { detectBulletTree } = await import('../../editor/paste');
    const block = makeBlockWithChildren();
    const serialized = serializeBlockTree(block);
    const parsed = detectBulletTree(serialized);
    expect(parsed).not.toBeNull();
    // 4 blocks total (parent + 2 children + 1 grandchild)
    expect(parsed!.items.length).toBe(4);
  });

  it('round-trip: depth array matches original tree structure', async () => {
    const { detectBulletTree } = await import('../../editor/paste');
    const block = makeBlockWithChildren();
    const serialized = serializeBlockTree(block);
    const parsed = detectBulletTree(serialized);
    expect(parsed!.items.map((i) => i.depth)).toEqual([0, 1, 1, 2]);
  });

  it('raw already includes leading TAB prefix + "- " + text + newline', () => {
    const block = makeBlock({ depth: 2, raw: '\t\t- deep block\n' });
    expect(serializeBlockTree(block)).toBe('\t\t- deep block\n');
  });
});

// ─── history-routing inverse wiring tests ─────────────────────────────────────

describe('applyInverse', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        blockSubtree: [],
        fileHash: 'newhash',
        dirtyBlockIds: [],
      }),
    });
    vi.stubGlobal('fetch', fetchMock);
    treeOpLog.clear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    treeOpLog.clear();
  });

  it('Indent op inverse calls PATCH /structure with depth restore', async () => {
    const { applyInverse } = await import('../../editor/history-routing');
    const op = { kind: 'Indent' as const, blockId: 42, prevDepth: 1 };
    await applyInverse(op, 'filehash123');

    expect(fetchMock).toHaveBeenCalledOnce();
    const call = fetchMock.mock.calls[0];
    expect(call[0]).toContain('/api/blocks/42/structure');
    expect(call[1]?.method).toBe('PATCH');
    const body = JSON.parse(call[1]?.body ?? '{}');
    expect(body.prevHash).toBe('filehash123');
    expect(body.depth).toBe(1);
  });

  it('Outdent op inverse calls PATCH /structure with depth restore', async () => {
    const { applyInverse } = await import('../../editor/history-routing');
    const op = { kind: 'Outdent' as const, blockId: 7, prevDepth: 0 };
    await applyInverse(op, 'filehash456');

    expect(fetchMock).toHaveBeenCalledOnce();
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body ?? '{}');
    expect(body.depth).toBe(0);
    expect(body.prevHash).toBe('filehash456');
  });

  it('Delete op inverse calls POST /blocks to restore snapshot', async () => {
    const { applyInverse } = await import('../../editor/history-routing');
    const op = {
      kind: 'Delete' as const,
      blockId: 10,
      snapshot: { raw: '- deleted\n', depth: 0, parentId: null, ord: 3 },
    };
    // Also need pageId — will need to be passed or handled
    await applyInverse(op, 'filehash789', 5);

    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock.mock.calls[0][0]).toContain('/api/blocks');
    expect(fetchMock.mock.calls[0][1]?.method).toBe('POST');
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body ?? '{}');
    expect(body.raw).toBe('- deleted\n');
    expect(body.prevHash).toBe('filehash789');
  });

  it('on 409 from applyInverse, op is restored to treeOpLog', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 409,
      json: async () => ({ error: 'stale', currentFileHash: 'newhash' }),
    });

    const { applyInverse } = await import('../../editor/history-routing');
    const op = { kind: 'Indent' as const, blockId: 1, prevDepth: 0 };

    // The function should restore the op on 409
    await applyInverse(op, 'hash', undefined, (restoredOp) => {
      treeOpLog.push(restoredOp);
    });

    const log = get(treeOpLog);
    expect(log.length).toBe(1);
    expect(log[0].kind).toBe('Indent');
  });
});

// ─── BulletPopover rendering tests ────────────────────────────────────────────

describe('BulletPopover', () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.removeChild(container);
  });

  it('renders 6 action items', async () => {
    const { default: BulletPopover } = await import('../BulletPopover.svelte');
    const block = makeBlock();
    const onClose = vi.fn();
    const onAction = vi.fn();

    const instance = mount(BulletPopover as any, {
      target: container,
      props: { block, onClose, onAction },
    });

    // Should have 6 menu items
    const items = container.querySelectorAll('[data-action]');
    expect(items.length).toBe(6);

    unmount(instance);
  });

  it('calls onAction when an item is clicked', async () => {
    const { default: BulletPopover } = await import('../BulletPopover.svelte');
    const block = makeBlock();
    const onClose = vi.fn();
    const onAction = vi.fn();

    const instance = mount(BulletPopover as any, {
      target: container,
      props: { block, onClose, onAction },
    });

    const firstItem = container.querySelector('[data-action]') as HTMLElement;
    expect(firstItem).not.toBeNull();
    firstItem.click();

    expect(onAction).toHaveBeenCalledOnce();

    unmount(instance);
  });

  it('calls onClose when Escape is pressed', async () => {
    const { default: BulletPopover } = await import('../BulletPopover.svelte');
    const block = makeBlock();
    const onClose = vi.fn();
    const onAction = vi.fn();

    const instance = mount(BulletPopover as any, {
      target: container,
      props: { block, onClose, onAction },
    });

    const escEvent = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
    document.dispatchEvent(escEvent);

    expect(onClose).toHaveBeenCalledOnce();

    unmount(instance);
  });
});
