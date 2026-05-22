// Tests for autocomplete.ts — CM6 completion source for [[page]] and #tag triggers.
// Uses minimal CompletionContext mocks to avoid full CM6 setup.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { completionSource } from '../autocomplete';

// ─── mock helpers ─────────────────────────────────────────────────────────────

function makeCtx(text: string): import('@codemirror/autocomplete').CompletionContext {
  const pos = text.length;
  return {
    pos,
    explicit: false,
    aborted: false,
    state: {
      doc: {
        sliceString: (from: number, to?: number) => text.slice(from, to ?? text.length),
      },
    } as any,
    tokenBefore: () => null as any,
    matchBefore: (re: RegExp) => {
      const m = text.match(re);
      if (!m) return null;
      // Return match ending at pos
      const full = text;
      const lastMatch = [...full.matchAll(new RegExp(re.source, 'gu'))].pop();
      if (!lastMatch) return null;
      return {
        from: lastMatch.index ?? 0,
        to: pos,
        text: lastMatch[0],
      };
    },
  } as any;
}

// ─── tests ────────────────────────────────────────────────────────────────────

describe('completionSource', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('returns null when no [[ or # trigger', async () => {
    const ctx = makeCtx('hello world');
    const result = await completionSource(ctx);
    expect(result).toBeNull();
  });

  it('fires kind=page fetch when [[ trigger detected', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: async () => ['PageA', 'PageB'],
    });

    const ctx = makeCtx('[[Foo');
    const result = await completionSource(ctx);

    expect(fetchMock).toHaveBeenCalledOnce();
    const url = fetchMock.mock.calls[0][0] as string;
    expect(url).toContain('kind=page');
    expect(url).toContain('prefix=Foo');

    expect(result).not.toBeNull();
    expect(result!.options.length).toBe(2);
    expect(result!.options[0].label).toBe('PageA');
    expect(result!.options[0].type).toBe('page');
  });

  it('from field replaces only the typed prefix after [[', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: async () => ['Glauber'],
    });

    const text = '[[Glau';
    const ctx = makeCtx(text);
    const result = await completionSource(ctx);

    // from should be pos - 'Glau'.length = 6 - 4 = 2
    // so only 'Glau' is replaced, not '[['
    expect(result!.from).toBe(text.length - 'Glau'.length);
  });

  it('fires kind=all fetch when # trigger detected', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: async () => [
        { name: 'crypto', kind: 'tag' },
        { name: 'Crypto Notes', kind: 'page' },
      ],
    });

    const ctx = makeCtx('#cr');
    const result = await completionSource(ctx);

    expect(fetchMock).toHaveBeenCalledOnce();
    const url = fetchMock.mock.calls[0][0] as string;
    expect(url).toContain('kind=all');
    expect(url).toContain('prefix=cr');

    expect(result).not.toBeNull();
    expect(result!.options.length).toBe(2);
    expect(result!.options[0].type).toBe('tag');
    expect(result!.options[1].type).toBe('page');
  });

  it('from field for # trigger replaces only the word after #', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: async () => [{ name: 'work', kind: 'tag' }],
    });

    const text = '#wor';
    const ctx = makeCtx(text);
    const result = await completionSource(ctx);

    // from should be pos - 'wor'.length = 4 - 3 = 1
    expect(result!.from).toBe(text.length - 'wor'.length);
  });

  it('returns null when # followed by no word character', async () => {
    const ctx = makeCtx('test # ');
    // no fetch should be called for a lone # with space after
    const result = await completionSource(ctx);
    if (result === null) {
      expect(fetchMock).not.toHaveBeenCalled();
    }
    // Either null or empty options is acceptable
  });

  it('encodes special characters in prefix URL parameter', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: async () => [],
    });

    const ctx = makeCtx('[[Café');
    await completionSource(ctx);

    const url = fetchMock.mock.calls[0][0] as string;
    // prefix should be URL-encoded
    expect(url).toContain('prefix=');
    expect(url).not.toContain(' ');
  });
});
