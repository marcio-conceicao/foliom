import { writable, type Writable } from 'svelte/store';
import type { PageDetail, PageSummary } from './api';

export type Theme = 'light' | 'dark' | 'auto';
const VALID_THEMES: ReadonlySet<Theme> = new Set<Theme>(['light', 'dark', 'auto']);

function coerceTheme(raw: string | null): Theme {
  return raw && VALID_THEMES.has(raw as Theme) ? (raw as Theme) : 'auto';
}

function createThemeStore(): Writable<Theme> {
  const initial: Theme =
    typeof localStorage !== 'undefined' ? coerceTheme(localStorage.getItem('theme')) : 'auto';
  const store = writable<Theme>(initial);
  if (typeof localStorage !== 'undefined') {
    store.subscribe((value) => {
      try {
        localStorage.setItem('theme', value);
      } catch {
        // Quota exceeded or storage disabled — silently ignore; theme is an aesthetic preference.
      }
    });
  }
  return store;
}

export const currentPage: Writable<PageDetail | null> = writable(null);
export const sidebarPages: Writable<PageSummary[]> = writable([]);
export const theme: Writable<Theme> = createThemeStore();
export const searchPalette: Writable<{ open: boolean; query: string }> = writable({
  open: false,
  query: '',
});
