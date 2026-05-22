<script lang="ts">
  import Router from 'svelte-spa-router';
  import { routes } from './routes';
  import { theme } from './lib/stores';
  import Sidebar from './lib/components/Sidebar.svelte';

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

  // 02-06 will add: search palette modal slot here (mounted as a sibling of
  // .layout, controlled by `searchPalette` store; visible via Ctrl/Cmd+K).
</script>

<div class="layout">
  <aside class="sidebar">
    <Sidebar />
  </aside>
  <main class="main">
    <Router {routes} />
  </main>
</div>
