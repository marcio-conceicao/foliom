// Sidebar tests — mounts Sidebar with a mocked `fetchPages` and asserts
// that page entries are grouped into "Páginas" vs "Journals" sections,
// alphabetized, and that unresolved entries carry the `.unresolved` class.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount, tick } from 'svelte';
import Sidebar from '../Sidebar.svelte';
import { sidebarPages } from '../../stores';

function newTarget(): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  return host;
}

describe('Sidebar.svelte', () => {
  beforeEach(() => {
    sidebarPages.set([]);
    // Reset hash so any nav side-effects in other tests don't leak in.
    window.location.hash = '';
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    sidebarPages.set([]);
  });

  it('loads pages via fetchPages on mount, groups into Pages + Journals, alphabetical NOCASE', async () => {
    const sample = [
      { name: 'beta', isJournal: false, isResolved: true },
      { name: 'Alpha', isJournal: false, isResolved: true },
      { name: '2024_03_15', isJournal: true, isResolved: true },
      { name: 'Ghost', isJournal: false, isResolved: false },
    ];
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(JSON.stringify(sample), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(Sidebar, { target, props: {} });
    // wait for the fetch microtask + svelte effect re-runs
    await new Promise<void>((r) => setTimeout(r, 0));
    await tick();

    expect(fetchMock).toHaveBeenCalledWith('/api/pages', expect.anything());

    const pagesSection = target.querySelector('[data-section="pages"]');
    const journalsSection = target.querySelector('[data-section="journals"]');
    expect(pagesSection).not.toBeNull();
    expect(journalsSection).not.toBeNull();

    const pageLinks = Array.from(pagesSection!.querySelectorAll('a'));
    const pageNames = pageLinks.map((a) => a.textContent?.trim());
    // Alphabetical case-insensitive: Alpha, beta, Ghost
    expect(pageNames).toEqual(['Alpha', 'beta', 'Ghost']);

    const journalLinks = Array.from(journalsSection!.querySelectorAll('a'));
    expect(journalLinks.map((a) => a.textContent?.trim())).toEqual(['2024_03_15']);

    // Unresolved styling: Ghost should carry .unresolved
    const ghost = pageLinks.find((a) => a.textContent?.trim() === 'Ghost');
    expect(ghost?.classList.contains('unresolved')).toBe(true);

    // hrefs use the hash router shape
    expect(ghost?.getAttribute('href')).toBe('#/pages/Ghost');

    unmount(app);
  });

  it('search input filters the list (case-insensitive substring)', async () => {
    const sample = [
      { name: 'Foo', isJournal: false, isResolved: true },
      { name: 'Bar', isJournal: false, isResolved: true },
      { name: 'Baz', isJournal: false, isResolved: true },
    ];
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(JSON.stringify(sample), { status: 200 }),
    );
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(Sidebar, { target, props: {} });
    await new Promise<void>((r) => setTimeout(r, 0));
    await tick();

    const input = target.querySelector('input[type="search"]') as HTMLInputElement | null;
    expect(input).not.toBeNull();

    input!.value = 'ba';
    input!.dispatchEvent(new Event('input', { bubbles: true }));
    // debounce window is 100ms in Sidebar.svelte
    await new Promise<void>((r) => setTimeout(r, 150));
    await tick();

    const visible = Array.from(target.querySelectorAll('[data-section="pages"] a')).map(
      (a) => a.textContent?.trim(),
    );
    expect(visible.sort()).toEqual(['Bar', 'Baz']);

    unmount(app);
  });
});
