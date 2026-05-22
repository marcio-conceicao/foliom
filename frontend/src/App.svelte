<script lang="ts">
  import Router from 'svelte-spa-router';
  import { routes } from './routes';
  import { theme } from './lib/stores';

  let currentTheme = $state<'light' | 'dark' | 'auto'>('auto');
  theme.subscribe((value) => {
    currentTheme = value;
  });

  $effect(() => {
    const resolved =
      currentTheme === 'auto'
        ? window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light'
        : currentTheme;
    document.documentElement.setAttribute('data-theme', resolved);
  });
</script>

<div class="layout">
  <aside class="sidebar">Sidebar (plan 02-05)</aside>
  <main class="content">
    <Router {routes} />
  </main>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: 260px 1fr;
    min-height: 100vh;
  }
  .sidebar {
    border-right: 1px solid var(--guide-color);
    padding: 1rem;
    background: var(--bg);
    color: var(--fg);
  }
  .content {
    padding: 1rem 2rem;
    background: var(--bg);
    color: var(--fg);
    overflow: auto;
  }
</style>
