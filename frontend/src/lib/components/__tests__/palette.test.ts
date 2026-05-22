// palette.test.ts — covers the search palette modal body: debounced
// query routing, snippet sanitization (T-02-20), keyboard navigation
// (arrows + enter), and the LNK-07 #block= deep-link contract.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount, tick } from 'svelte';
import { get } from 'svelte/store';
import SearchPalette from '../SearchPalette.svelte';
import { searchPalette } from '../../stores';

function newTarget(): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  return host;
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

describe('SearchPalette.svelte', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    searchPalette.set({ open: true, query: '' });
    window.location.hash = '';
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    searchPalette.set({ open: false, query: '' });
    document.body.innerHTML = '';
  });

  it('debounces input for 150ms then calls /api/search with kind=content', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse([]));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();

    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    expect(input).not.toBeNull();
    input.value = 'Glauber';
    input.dispatchEvent(new Event('input', { bubbles: true }));

    // Not fired yet — still inside debounce window
    await vi.advanceTimersByTimeAsync(100);
    expect(fetchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(60); // total 160ms
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const url = fetchMock.mock.calls[0][0] as string;
    expect(url).toContain('/api/search?');
    expect(url).toContain('q=Glauber');
    expect(url).toContain('kind=content');
    expect(url).toContain('limit=50');

    unmount(app);
  });

  it('leading "#" routes the query to kind=tag and strips the prefix', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse([]));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = '#urgente';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);

    const url = fetchMock.mock.calls[0][0] as string;
    expect(url).toContain('kind=tag');
    expect(url).toContain('q=urgente');
    expect(url).not.toContain('q=%23'); // # must be stripped, not URL-encoded

    unmount(app);
  });

  it('leading "[[" calls /api/page-titles and filters client-side', async () => {
    const titles = ['Glauber', 'Avaliação', 'Foo', 'Glow'];
    const fetchMock = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/api/page-titles')) return Promise.resolve(jsonResponse(titles));
      return Promise.resolve(jsonResponse([]));
    });
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = '[[gl';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);
    // Allow the page-titles promise resolution to land.
    await vi.runAllTimersAsync();
    await tick();

    const calls = fetchMock.mock.calls.map((c) => c[0]);
    expect(calls.some((u: string) => u.includes('/api/page-titles'))).toBe(true);
    // Should NOT hit /api/search for the [[ branch
    expect(calls.some((u: string) => u.includes('/api/search'))).toBe(false);

    const rendered = Array.from(target.querySelectorAll('li[data-result]')).map(
      (li) => li.textContent ?? '',
    );
    // Glauber + Glow match "gl" (case-insensitive); Foo and Avaliação do not.
    const text = rendered.join('|');
    expect(text).toContain('Glauber');
    expect(text).toContain('Glow');
    expect(text).not.toContain('Foo');

    unmount(app);
  });

  it('renders snippet with <mark> preserved and strips every other tag (XSS allow-list)', async () => {
    const hits = [
      {
        page: 'Diário',
        blockId: 4272,
        snippet: 'foo <mark>Glauber</mark> bar <script>alert(1)</script>',
      },
    ];
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(hits));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'Glauber';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);
    await tick();

    const snippet = target.querySelector('.snippet');
    expect(snippet).not.toBeNull();
    // <mark> must survive
    expect(snippet!.querySelector('mark')?.textContent).toBe('Glauber');
    // <script> must NOT be present as an element
    expect(snippet!.querySelector('script')).toBeNull();
    // ...and the literal text "alert(1)" should appear escaped (as text)
    expect(snippet!.textContent).toContain('alert(1)');

    unmount(app);
  });

  it('Enter on a highlighted result sets location.hash to #/pages/<page>#block=<blockId> and closes', async () => {
    const hits = [
      { page: '2024_03_15', blockId: 4272, snippet: 'first' },
      { page: 'Other', blockId: 7, snippet: 'second' },
    ];
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(hits));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'Glauber';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);
    await tick();

    // Default cursor is the first row. Enter should navigate to it.
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    );
    // Let the requestAnimationFrame fire that sets the hash.
    await vi.runAllTimersAsync();
    await tick();

    expect(window.location.hash).toBe('#/pages/2024_03_15#block=4272');
    expect(get(searchPalette).open).toBe(false);

    unmount(app);
  });

  it('ArrowDown then Enter navigates to the SECOND result', async () => {
    const hits = [
      { page: 'First', blockId: 1, snippet: 'a' },
      { page: 'Second', blockId: 2, snippet: 'b' },
    ];
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(hits));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'x';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);
    await tick();

    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    );
    await tick();
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    );
    await vi.runAllTimersAsync();
    await tick();

    expect(window.location.hash).toBe('#/pages/Second#block=2');

    unmount(app);
  });

  it('empty result renders "Sem resultados para" in Portuguese', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse([]));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'nothingmatches';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);
    await tick();

    const empty = target.querySelector('.empty');
    expect(empty?.textContent).toMatch(/Sem resultados para/);
    expect(empty?.textContent).toContain('nothingmatches');

    unmount(app);
  });

  it('empty/whitespace input clears results without firing a fetch', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse([]));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = '   ';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(target.querySelectorAll('li[data-result]').length).toBe(0);

    unmount(app);
  });

  it('non-numeric blockId is sanitized — fragment omits #block= (T-02-21)', async () => {
    const hits = [{ page: 'Broken', blockId: 'evil', snippet: 'x' } as unknown];
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(hits));
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(SearchPalette, { target, props: { mode: 'modal' } });
    await tick();
    const input = target.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'x';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(200);
    await tick();

    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    );
    await vi.runAllTimersAsync();
    await tick();

    // Hash points at the page but does NOT include a #block=NaN fragment
    expect(window.location.hash).toBe('#/pages/Broken');

    unmount(app);
  });
});
