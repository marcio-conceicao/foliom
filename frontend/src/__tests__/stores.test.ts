import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';

describe('stores', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('currentPage starts as null', async () => {
    const { currentPage } = await import('../lib/stores');
    expect(get(currentPage)).toBeNull();
  });

  it('theme persists writes to localStorage', async () => {
    // Re-import to pick up a clean module if needed (vitest isolates per file).
    const { theme } = await import('../lib/stores');
    theme.set('dark');
    expect(localStorage.getItem('theme')).toBe('dark');
    theme.set('light');
    expect(localStorage.getItem('theme')).toBe('light');
  });

  it('searchPalette has initial closed state', async () => {
    const { searchPalette } = await import('../lib/stores');
    expect(get(searchPalette)).toEqual({ open: false, query: '' });
  });
});
