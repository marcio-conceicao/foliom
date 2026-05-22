<script lang="ts">
  import Router from 'svelte-spa-router';
  import { routes } from './routes';
  import { theme, searchPalette } from './lib/stores';
  import Sidebar from './lib/components/Sidebar.svelte';
  import SearchPalette from './lib/components/SearchPalette.svelte';
  import { bindGlobalShortcuts } from './lib/keys';

  // Theme resolution lives at the App level so it (a) reacts to user
  // selection via the ThemeToggle (which writes the `theme` store) and
  // (b) reacts to OS-level `prefers-color-scheme` changes when set to
  // "auto". The ThemeToggle component also sets data-theme eagerly on
  // click so the change feels instant; this $effect is the authoritative
  // sync loop.
  let currentTheme = $state<'light' | 'dark' | 'auto'>('auto');
  theme.subscribe((value) => {
    currentTheme = value;
  });

  // Recompute the resolved theme whenever currentTheme changes or whenever
  // the OS reports a prefers-color-scheme flip (only relevant under "auto").
  let mqlMatches = $state(false);
  $effect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return;
    const mql = window.matchMedia('(prefers-color-scheme: dark)');
    mqlMatches = mql.matches;
    const handler = (e: MediaQueryListEvent) => {
      mqlMatches = e.matches;
    };
    mql.addEventListener('change', handler);
    return () => mql.removeEventListener('change', handler);
  });

  $effect(() => {
    const resolved =
      currentTheme === 'auto' ? (mqlMatches ? 'dark' : 'light') : currentTheme;
    document.documentElement.setAttribute('data-theme', resolved);
  });

  // Global keymap (SCH-03): Ctrl/Cmd+K toggles the search palette;
  // Esc closes it. Returning the disposer from $effect keeps HMR happy
  // and prevents duplicate listener registration on re-mount.
  $effect(() => bindGlobalShortcuts());

  // searchPalette store gate — keeps the palette out of the DOM entirely
  // while closed (modal chrome incl. backdrop only mounts on demand).
  let paletteOpen = $state(false);
  searchPalette.subscribe((s) => {
    paletteOpen = s.open;
  });
</script>

<div class="layout">
  <aside class="sidebar">
    <Sidebar />
  </aside>
  <main class="main">
    <Router {routes} />
  </main>
</div>

{#if paletteOpen}
  <SearchPalette mode="modal" />
{/if}
