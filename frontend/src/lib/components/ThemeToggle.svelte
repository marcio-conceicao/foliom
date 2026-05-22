<script lang="ts">
  // Tri-state theme switcher. Writes to the `theme` store (which persists to
  // localStorage), and also eagerly applies the resolved `data-theme` to
  // `<html>` so the toggle "just works" even outside the App.svelte mount
  // (notably under vitest where App.svelte's $effect isn't running).
  //
  // The App-level $effect in App.svelte is still the authoritative source
  // of truth for theme resolution during a real run (it also reacts to
  // `prefers-color-scheme` change events); this component just keeps the
  // attribute in sync so a click feels instant.
  import { theme, type Theme } from '../stores';

  let current = $state<Theme>('auto');
  theme.subscribe((value) => {
    current = value;
  });

  function resolve(t: Theme): 'light' | 'dark' {
    if (t === 'auto') {
      return typeof window !== 'undefined' &&
        window.matchMedia &&
        window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light';
    }
    return t;
  }

  function pick(t: Theme): void {
    theme.set(t);
    if (typeof document !== 'undefined') {
      document.documentElement.setAttribute('data-theme', resolve(t));
    }
  }
</script>

<div class="theme-toggle" role="group" aria-label="Tema">
  <button
    type="button"
    aria-pressed={current === 'light'}
    class:active={current === 'light'}
    onclick={() => pick('light')}
  >Claro</button>
  <button
    type="button"
    aria-pressed={current === 'auto'}
    class:active={current === 'auto'}
    onclick={() => pick('auto')}
  >Auto</button>
  <button
    type="button"
    aria-pressed={current === 'dark'}
    class:active={current === 'dark'}
    onclick={() => pick('dark')}
  >Escuro</button>
</div>

<style>
  .theme-toggle {
    display: inline-flex;
    gap: 0;
    border: 1px solid var(--guide-color);
    border-radius: 0.4rem;
    overflow: hidden;
    font-size: 0.85rem;
  }
  .theme-toggle button {
    background: none;
    color: var(--fg);
    border: 0;
    padding: 0.25rem 0.6rem;
    cursor: pointer;
    line-height: 1.3;
  }
  .theme-toggle button + button {
    border-left: 1px solid var(--guide-color);
  }
  .theme-toggle button.active {
    background: var(--tag-bg);
    color: var(--tag-fg);
  }
  .theme-toggle button:hover:not(.active) {
    background: var(--code-bg);
  }
</style>
