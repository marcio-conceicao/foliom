// BacklinksPanel tests — mocks fetchBacklinks, mounts the panel for a
// page name, asserts grouped <h3> per source page and one <a> per
// backlink with `#block=<id>` in the href fragment.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount, tick } from 'svelte';
import BacklinksPanel from '../BacklinksPanel.svelte';

function newTarget(): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  return host;
}

describe('BacklinksPanel.svelte', () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('groups backlinks by source page and emits #block=<id> hrefs', async () => {
    const sample = [
      { page: 'Alpha', blockId: 10, snippet: '\t- mention of [[Foo]] here\n' },
      { page: 'Alpha', blockId: 12, snippet: '\t- another [[Foo]]\n' },
      { page: 'Beta', blockId: 5, snippet: '\t- [[Foo]] referenced too\n' },
    ];
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(JSON.stringify(sample), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(BacklinksPanel, { target, props: { name: 'Foo' } });
    await new Promise<void>((r) => setTimeout(r, 0));
    await tick();

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/pages/Foo/backlinks',
      expect.anything(),
    );

    const details = target.querySelector('details');
    expect(details).not.toBeNull();
    // open by default
    expect(details!.hasAttribute('open')).toBe(true);

    // summary contains the count
    expect(details!.querySelector('summary')?.textContent).toMatch(/Backlinks\s*\(3\)/);

    // Two source-page groupings
    const groups = details!.querySelectorAll('h3');
    expect(groups.length).toBe(2);
    const groupNames = Array.from(groups).map((h) => h.textContent?.trim());
    expect(groupNames.sort()).toEqual(['Alpha', 'Beta']);

    // Three backlink anchors total
    const anchors = details!.querySelectorAll('a');
    expect(anchors.length).toBe(3);
    const hrefs = Array.from(anchors).map((a) => a.getAttribute('href'));
    expect(hrefs).toContain('#/pages/Alpha#block=10');
    expect(hrefs).toContain('#/pages/Alpha#block=12');
    expect(hrefs).toContain('#/pages/Beta#block=5');

    // Snippet stripped of the segmenter prefix — no leading tab or "- "
    const snippetTexts = Array.from(anchors).map((a) => a.textContent?.trim());
    snippetTexts.forEach((s) => {
      expect(s?.startsWith('-')).toBe(false);
      expect(s?.startsWith('\t')).toBe(false);
    });

    unmount(app);
  });

  it('renders "Sem backlinks" when fetch returns []', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response('[]', { status: 200, headers: { 'Content-Type': 'application/json' } }),
    );
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(BacklinksPanel, { target, props: { name: 'Lonely' } });
    await new Promise<void>((r) => setTimeout(r, 0));
    await tick();

    const empty = target.querySelector('.empty');
    expect(empty?.textContent).toMatch(/Sem backlinks/);

    unmount(app);
  });

  it('snippet text is escaped (no HTML interpretation)', async () => {
    const malicious = [
      { page: 'Evil', blockId: 1, snippet: '\t- <script>alert(1)</script>\n' },
    ];
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(JSON.stringify(malicious), { status: 200 }),
    );
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(BacklinksPanel, { target, props: { name: 'Target' } });
    await new Promise<void>((r) => setTimeout(r, 0));
    await tick();

    // The anchor should contain literal "<script>" as text, not a child <script> element.
    const anchor = target.querySelector('a');
    expect(anchor?.textContent).toContain('<script>');
    expect(anchor?.querySelector('script')).toBeNull();

    unmount(app);
  });
});
