import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  fetchPages,
  fetchPage,
  fetchBacklinks,
  fetchPageTitles,
  fetchSearch,
  fetchJournalsRange,
} from '../lib/api';

type FetchMock = ReturnType<typeof vi.fn>;

function mockJson(body: unknown, init: Partial<{ ok: boolean; status: number; url: string }> = {}) {
  return {
    ok: init.ok ?? true,
    status: init.status ?? 200,
    statusText: 'OK',
    url: init.url ?? '',
    json: async () => body,
  } as unknown as Response;
}

describe('api wrappers', () => {
  let fetchMock: FetchMock;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('fetchPages hits /api/pages and returns parsed array', async () => {
    const sample = [{ name: 'Foo', isJournal: false, isResolved: true }];
    fetchMock.mockResolvedValueOnce(mockJson(sample));
    const result = await fetchPages();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe('/api/pages');
    expect(result).toEqual(sample);
  });

  it('fetchPage encodes slashes in page name (Parent/Child → Parent%2FChild)', async () => {
    fetchMock.mockResolvedValueOnce(
      mockJson({ name: 'Parent/Child', isJournal: false, formattedTitle: null, blocks: [] }),
    );
    await fetchPage('Parent/Child');
    expect(fetchMock.mock.calls[0][0]).toBe('/api/pages/Parent%2FChild');
  });

  it('fetchBacklinks builds /api/pages/{enc}/backlinks', async () => {
    fetchMock.mockResolvedValueOnce(mockJson([]));
    await fetchBacklinks('A B');
    expect(fetchMock.mock.calls[0][0]).toBe('/api/pages/A%20B/backlinks');
  });

  it('fetchPageTitles hits /api/page-titles', async () => {
    fetchMock.mockResolvedValueOnce(mockJson(['A', 'B']));
    const titles = await fetchPageTitles();
    expect(fetchMock.mock.calls[0][0]).toBe('/api/page-titles');
    expect(titles).toEqual(['A', 'B']);
  });

  it('fetchSearch builds URL with q + limit + optional kind', async () => {
    fetchMock.mockResolvedValueOnce(mockJson([]));
    await fetchSearch('hello world', 'tag', 5);
    const url = fetchMock.mock.calls[0][0] as string;
    expect(url.startsWith('/api/search?')).toBe(true);
    expect(url).toContain('q=hello+world');
    expect(url).toContain('limit=5');
    expect(url).toContain('kind=tag');
  });

  it('fetchSearch omits kind when not given and defaults limit to 20', async () => {
    fetchMock.mockResolvedValueOnce(mockJson([]));
    await fetchSearch('x');
    const url = fetchMock.mock.calls[0][0] as string;
    expect(url).toContain('limit=20');
    expect(url).not.toContain('kind=');
  });

  it('fetchJournalsRange passes from/to query params', async () => {
    fetchMock.mockResolvedValueOnce(mockJson([]));
    await fetchJournalsRange('2026-05-01', '2026-05-31');
    const url = fetchMock.mock.calls[0][0] as string;
    expect(url.startsWith('/api/journals?')).toBe(true);
    expect(url).toContain('from=2026-05-01');
    expect(url).toContain('to=2026-05-31');
  });

  it('throws on non-2xx response', async () => {
    fetchMock.mockResolvedValueOnce(mockJson(null, { ok: false, status: 500 }));
    await expect(fetchPages()).rejects.toThrow(/500/);
  });
});
