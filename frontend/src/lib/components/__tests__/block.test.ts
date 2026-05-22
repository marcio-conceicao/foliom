import { describe, it, expect } from 'vitest';
import { mount, unmount } from 'svelte';
import Block from '../Block.svelte';
import { buildThousandBlockTree } from '../../markdown/__tests__/fixtures/1000-blocks';

function newTarget(): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  return host;
}

describe('Block.svelte', () => {
  it('renders a single block with bold inline + bullet chrome', () => {
    const target = newTarget();
    const app = mount(Block, {
      target,
      props: {
        id: 42,
        depth: 0,
        raw: '- Hello **world**\n',
        properties: [],
        drawers: [],
        children: [],
      },
    });
    expect(target.querySelector('[id="block-42"]')).not.toBeNull();
    expect(target.querySelector('.content')?.innerHTML).toContain('<strong>world</strong>');
    expect(target.querySelector('.fold-toggle')).not.toBeNull();
    unmount(app);
  });

  it('emits page-link chip with data-page attribute', () => {
    const target = newTarget();
    const app = mount(Block, {
      target,
      props: {
        id: 1,
        depth: 0,
        raw: '- See [[Glauber]] now\n',
        properties: [],
        drawers: [],
        children: [],
      },
    });
    const link = target.querySelector('a.page-link') as HTMLAnchorElement | null;
    expect(link).not.toBeNull();
    expect(link!.dataset.page).toBe('Glauber');
    expect(link!.getAttribute('href')).toBe('#/pages/Glauber');
    unmount(app);
  });

  it('prelude (depth -1) renders children without bullet chrome', () => {
    const target = newTarget();
    const app = mount(Block, {
      target,
      props: {
        id: 1,
        depth: -1,
        raw: '',
        properties: [],
        drawers: [],
        children: [
          {
            id: 2,
            depth: 0,
            raw: '- child\n',
            properties: [],
            drawers: [],
            children: [],
          },
        ],
      },
    });
    expect(target.querySelector('[id="block-1"]')).toBeNull();
    expect(target.querySelector('[id="block-2"]')).not.toBeNull();
    unmount(app);
  });

  it('renders nested children inside .children container', () => {
    const target = newTarget();
    const app = mount(Block, {
      target,
      props: {
        id: 1,
        depth: 0,
        raw: '- parent\n',
        properties: [],
        drawers: [],
        children: [
          {
            id: 2,
            depth: 1,
            raw: '\t- child\n',
            properties: [],
            drawers: [],
            children: [],
          },
        ],
      },
    });
    const children = target.querySelector('.children');
    expect(children).not.toBeNull();
    expect(children!.querySelector('[id="block-2"]')).not.toBeNull();
    unmount(app);
  });

  // 1000-block soft perf assertion. happy-dom has high-res `performance.now()`
  // so we run the assertion; if precision is unexpectedly low, skip.
  const hasHiRes =
    typeof performance !== 'undefined' && typeof performance.now === 'function';
  it.skipIf(!hasHiRes)(
    'renders a 1000-block tree under the soft perf ceiling',
    () => {
      const tree = buildThousandBlockTree();
      const target = newTarget();
      const t0 = performance.now();
      const app = mount(Block, {
        target,
        props: {
          id: 0,
          depth: -1,
          raw: '',
          properties: [],
          drawers: [],
          children: tree,
        },
      });
      const t1 = performance.now();
      const blocks = target.querySelectorAll('.block');
      // 10 top-level + 1000 leaf children = 1010 blocks rendered.
      expect(blocks.length).toBe(1010);
      // CI ceiling — happy-dom is slower than a real browser but still
      // well under 2000ms for this fixture.
      expect(t1 - t0).toBeLessThan(2000);
      unmount(app);
    },
  );
});
