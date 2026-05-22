import { describe, it, expect } from 'vitest';
import { stripForRender } from '../strip';

describe('stripForRender — segmenter prefix', () => {
  it('returns empty string for prelude (depth < 0)', () => {
    expect(stripForRender('anything', -1, [], [])).toBe('');
  });

  it('strips a single-line "\\t- Foo\\n" at depth 0 (no leading TABs needed)', () => {
    expect(stripForRender('\t- Foo\n', 0, [], [])).toBe('Foo\n');
  });

  it('strips bullet + continuation marker across multi-line block', () => {
    const raw = '\t- Reunião\n\t  com [[Glauber]]\n';
    expect(stripForRender(raw, 0, [], [])).toBe('Reunião\ncom [[Glauber]]\n');
  });

  it('strips deeper depth with multiple leading TABs', () => {
    const raw = '\t\t- Nested\n\t\t  body\n';
    expect(stripForRender(raw, 1, [], [])).toBe('Nested\nbody\n');
  });

  it('drops `key:: value` property lines', () => {
    const raw = '\t- Title\n\t  id:: abc-123\n\t  collapsed:: true\n\t  body line\n';
    expect(stripForRender(raw, 0, [], [])).toBe('Title\nbody line\n');
  });

  it('drops drawer ranges (`:LOGBOOK:` ... `:END:` inclusive)', () => {
    const raw = '\t- Header\n\t  :LOGBOOK:\n\t  CLOCK: [2024]\n\t  :END:\n\t  After drawer\n';
    expect(stripForRender(raw, 0, [], [])).toBe('Header\nAfter drawer\n');
  });

  it('passes a non-matching first line through (no `- ` marker)', () => {
    expect(stripForRender('plain text\n', 0, [], [])).toBe('plain text\n');
  });
});
