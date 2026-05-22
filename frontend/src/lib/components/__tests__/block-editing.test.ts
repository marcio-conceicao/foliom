// Tests for Block.svelte editing state and api.ts mutation wrappers.
// Tests the click-to-edit flow, 409 banner trigger, and API wrapper behavior.

import { mount, unmount } from 'svelte';
import { get } from 'svelte/store';
import { expect, test, vi, describe, beforeEach, afterEach } from 'vitest';
import Block from '../../components/Block.svelte';
import { currentlyEditing } from '../../stores/editing';
import { treeOpLog } from '../../stores/treeOpLog';
import * as api from '../../api';
import type { Block as BlockData } from '../../api';

beforeEach(() => {
  document.body.innerHTML = '';
  treeOpLog.clear();
  currentlyEditing.set(null);
});

afterEach(() => {
  document.body.innerHTML = '';
  treeOpLog.clear();
  currentlyEditing.set(null);
  vi.restoreAllMocks();
});

function makeBlock(overrides: Partial<BlockData> = {}): BlockData {
  return {
    id: 1,
    depth: 0,
    raw: '- hello world',
    properties: [],
    drawers: [],
    children: [],
    ...overrides,
  };
}

function mountBlock(block: BlockData, fileHash = 'abc123') {
  const target = document.createElement('div');
  document.body.appendChild(target);
  const component = mount(Block, {
    target,
    props: { ...block, fileHash },
  });
  return { target, component };
}

describe('Block.svelte — click-to-edit (EDT-01)', () => {
  test('clicking .content mounts CM6 editor and adds .editing class', async () => {
    const { target } = mountBlock(makeBlock());
    const blockEl = target.querySelector('.block') as HTMLElement;
    const contentEl = target.querySelector('.content') as HTMLElement;

    expect(blockEl).not.toBeNull();
    expect(contentEl).not.toBeNull();

    // Click on content (not a chip)
    contentEl.click();
    // Need a tick for Svelte reactivity
    await new Promise((r) => setTimeout(r, 0));

    // The block should have .editing class
    expect(blockEl.classList.contains('editing')).toBe(true);
    // CM6 should be mounted
    expect(target.querySelector('.cm-content')).not.toBeNull();
  });

  test('currentlyEditing store is set to the block id on click', async () => {
    mountBlock(makeBlock({ id: 42 }));
    const contentEl = document.querySelector('.content') as HTMLElement;
    contentEl.click();
    await new Promise((r) => setTimeout(r, 0));
    expect(get(currentlyEditing)).toBe(42);
  });
});

describe('api.ts mutation wrappers', () => {
  test('putBlock sends PUT /api/blocks/:id with raw and prevHash', async () => {
    const { putBlock } = await import('../../api');
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          blockSubtree: [],
          fileHash: 'newhash',
          dirtyBlockIds: [1],
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    const result = await putBlock(1, '- updated text', 'oldhash');
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blocks/1',
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({ raw: '- updated text', prevHash: 'oldhash' }),
      }),
    );
    expect(result).toMatchObject({ fileHash: 'newhash' });
  });

  test('putBlock returns stale object on 409', async () => {
    const { putBlock } = await import('../../api');
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({ error: 'stale', currentFileHash: 'serverHash' }),
        { status: 409, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    const result = await putBlock(1, '- text', 'clientHash');
    expect(result).toEqual({ stale: true, currentFileHash: 'serverHash' });
  });

  test('deleteBlock sends DELETE /api/blocks/:id?prevHash=...', async () => {
    const { deleteBlock } = await import('../../api');
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({ blockSubtree: [], fileHash: 'newHash', dirtyBlockIds: [] }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    await deleteBlock(5, 'prevhashvalue');
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blocks/5?prevHash=prevhashvalue',
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  test('postBlock sends POST /api/blocks', async () => {
    const { postBlock } = await import('../../api');
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({ id: 99, blockSubtree: [], fileHash: 'hash99' }),
        { status: 201, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    await postBlock({ pageId: 1, parentId: null, ord: 1, depth: 0, raw: '- new', prevHash: 'h' });
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blocks',
      expect.objectContaining({
        method: 'POST',
        body: expect.stringContaining('"raw":"- new"'),
      }),
    );
  });

  test('patchBlockStructure sends PATCH /api/blocks/:id/structure', async () => {
    const { patchBlockStructure } = await import('../../api');
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({ blockSubtree: [], fileHash: 'hash', dirtyBlockIds: [] }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    await patchBlockStructure(3, { op: 'indent', prevHash: 'prevhash' });
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blocks/3/structure',
      expect.objectContaining({
        method: 'PATCH',
        body: expect.stringContaining('"op":"indent"'),
      }),
    );
  });
});
