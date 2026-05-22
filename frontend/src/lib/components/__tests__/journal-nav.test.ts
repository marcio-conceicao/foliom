// JournalNavigator tests — mounts the calendar fixed to a known month, clicks
// a day cell, asserts the resulting hash matches `#/journals/YYYY-MM-DD`.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mount, unmount, tick } from 'svelte';
import JournalNavigator from '../JournalNavigator.svelte';

function newTarget(): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  return host;
}

describe('JournalNavigator.svelte', () => {
  beforeEach(() => {
    window.location.hash = '';
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    window.location.hash = '';
  });

  it('renders a month grid pinned to the `initialMonth` prop and navigates on day click', async () => {
    const target = newTarget();
    // Pin to March 2024 — a non-current month so the test is deterministic.
    const app = mount(JournalNavigator, {
      target,
      props: { initialMonth: '2024-03' },
    });
    await tick();

    // Header should announce the month
    const header = target.querySelector('[data-role="month-label"]');
    expect(header?.textContent).toMatch(/2024/);

    const day15 = target.querySelector('[data-date="2024-03-15"]') as HTMLButtonElement | null;
    expect(day15).not.toBeNull();
    day15!.click();
    await tick();

    expect(window.location.hash).toBe('#/journals/2024-03-15');

    unmount(app);
  });

  it('arrow buttons step month-by-month', async () => {
    const target = newTarget();
    const app = mount(JournalNavigator, {
      target,
      props: { initialMonth: '2024-03' },
    });
    await tick();

    const next = target.querySelector('[data-role="next-month"]') as HTMLButtonElement;
    expect(next).not.toBeNull();
    next.click();
    await tick();

    // April 2024 should now be visible — day 30 exists in April, day 31 does not.
    expect(target.querySelector('[data-date="2024-04-30"]')).not.toBeNull();
    expect(target.querySelector('[data-date="2024-04-31"]')).toBeNull();

    const prev = target.querySelector('[data-role="prev-month"]') as HTMLButtonElement;
    prev.click();
    prev.click();
    await tick();
    // Now in February 2024 — leap year, day 29 exists, day 30 does not.
    expect(target.querySelector('[data-date="2024-02-29"]')).not.toBeNull();
    expect(target.querySelector('[data-date="2024-02-30"]')).toBeNull();

    unmount(app);
  });

  it('"Hoje" button uses /api/journals/today and navigates to the resolved name', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(null, {
        status: 200,
        // The actual server replies 302 with Location, but fetch with the
        // default redirect="follow" exposes the final URL via response.url.
        // We simulate the post-redirect URL here.
      }),
    );
    // Force a specific response URL to mimic the post-redirect target.
    Object.defineProperty(fetchMock.mock.results[0]?.value ?? {}, 'url', {
      value: '/api/pages/2026_05_21',
    });
    // Simpler: mock fetch to return an object with a `url` field already set.
    const responseWithUrl = {
      ok: true,
      status: 200,
      statusText: 'OK',
      url: '/api/pages/2026_05_21',
      headers: new Headers(),
      json: async () => ({}),
    } as unknown as Response;
    fetchMock.mockReset();
    fetchMock.mockResolvedValueOnce(responseWithUrl);
    vi.stubGlobal('fetch', fetchMock);

    const target = newTarget();
    const app = mount(JournalNavigator, {
      target,
      props: { initialMonth: '2024-03' },
    });
    await tick();

    const today = target.querySelector('[data-role="today"]') as HTMLButtonElement;
    expect(today).not.toBeNull();
    today.click();
    await new Promise<void>((r) => setTimeout(r, 0));
    await tick();

    expect(fetchMock).toHaveBeenCalledWith('/api/journals/today', expect.anything());
    // Today's resolved name maps to journal route — 2026_05_21 → 2026-05-21
    expect(window.location.hash).toBe('#/journals/2026-05-21');

    unmount(app);
  });
});
