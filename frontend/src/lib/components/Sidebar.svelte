<script lang="ts">
  // Left rail (LNK-06). Loads `/api/pages` once into the `sidebarPages`
  // store and renders two grouped sections (Pages, Journals). Includes a
  // small debounced search filter + placeholder sections for Favorites
  // and Recents (deferred per 02-CONTEXT §Deferred — v1.x).
  //
  // Theme toggle lives in the sidebar footer.
  //
  // 02-06 will add: "Buscar (Ctrl+K)" trigger button that opens the
  // searchPalette modal — leave the footer slot open for that.

  import { fetchPages, type PageSummary } from '../api';
  import { sidebarPages, searchPalette } from '../stores';
  import JournalNavigator from './JournalNavigator.svelte';
  import ThemeToggle from './ThemeToggle.svelte';
  import { watcherStatus } from '../stores/watcher';

  function openSearch(): void {
    searchPalette.set({ open: true, query: '' });
  }

  let pages = $state<PageSummary[]>([]);
  let error = $state<string | null>(null);
  let query = $state('');
  let debouncedQuery = $state('');

  // Hydrate from the store first (subsequent mounts during navigation
  // shouldn't re-fetch), then fall back to a fetch if empty.
  sidebarPages.subscribe((p) => {
    pages = p;
  });

  $effect(() => {
    if (pages.length > 0) return;
    fetchPages()
      .then((p) => {
        sidebarPages.set(p);
        pages = p;
      })
      .catch((e: unknown) => {
        error = e instanceof Error ? e.message : String(e);
      });
  });

  let debounceTimer: number | undefined;
  function onQueryInput(e: Event): void {
    const target = e.target as HTMLInputElement;
    query = target.value;
    if (debounceTimer !== undefined) clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      debouncedQuery = query;
    }, 100);
  }

  function matches(p: PageSummary, q: string): boolean {
    if (!q) return true;
    return p.name.toLowerCase().includes(q.toLowerCase());
  }

  const filteredPages = $derived(
    pages
      .filter((p) => !p.isJournal && matches(p, debouncedQuery))
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })),
  );
  const filteredJournals = $derived(
    pages
      .filter((p) => p.isJournal && matches(p, debouncedQuery))
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })),
  );
</script>

<nav class="sidebar-nav" aria-label="Navegação principal">
  <header class="brand">
    <strong>Foliom</strong>
  </header>

  <div class="filter">
    <input
      type="search"
      placeholder="Filtrar páginas…"
      value={query}
      oninput={onQueryInput}
      aria-label="Filtrar páginas"
    />
  </div>

  <section class="group">
    <h2>Favoritos</h2>
    <p class="empty"><small>vazio em v1.x</small></p>
  </section>

  <section class="group">
    <h2>Recentes</h2>
    <p class="empty"><small>vazio em v1.x</small></p>
  </section>

  <section class="group">
    <h2>Journals</h2>
    <JournalNavigator />
    <ul data-section="journals">
      {#each filteredJournals as p (p.name)}
        <li>
          <a
            class="page-link"
            class:unresolved={!p.isResolved}
            href={`#/pages/${encodeURIComponent(p.name)}`}
            data-page={p.name}
          >{p.name}</a>
        </li>
      {/each}
      {#if filteredJournals.length === 0}
        <li class="empty"><small>Nenhum journal.</small></li>
      {/if}
    </ul>
  </section>

  <section class="group">
    <h2>Páginas</h2>
    <ul data-section="pages">
      {#each filteredPages as p (p.name)}
        <li>
          <a
            class="page-link"
            class:unresolved={!p.isResolved}
            href={`#/pages/${encodeURIComponent(p.name)}`}
            data-page={p.name}
          >{p.name}</a>
        </li>
      {/each}
      {#if filteredPages.length === 0}
        <li class="empty"><small>Nenhuma página.</small></li>
      {/if}
    </ul>
  </section>

  {#if error}
    <p class="error">Não foi possível carregar as páginas: {error}</p>
  {/if}

  <footer class="footer">
    <button class="search-trigger" type="button" onclick={openSearch}>
      <span>Buscar</span>
      <kbd>Ctrl+K</kbd>
    </button>
    <ThemeToggle />
    <div class="watcher-pill" title="Watcher: {$watcherStatus}">
      <span class="watcher-dot" data-status={$watcherStatus}></span>
    </div>
  </footer>
</nav>

<style>
  .sidebar-nav {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 0.6rem;
    font-size: 0.92rem;
  }
  .brand {
    font-size: 1.1rem;
    padding-bottom: 0.25rem;
    border-bottom: 1px solid var(--guide-color);
  }
  .filter input {
    width: 100%;
    background: var(--code-bg);
    color: var(--fg);
    border: 1px solid var(--guide-color);
    border-radius: 0.3rem;
    padding: 0.3rem 0.5rem;
    font-size: 0.88rem;
  }
  .group h2 {
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg);
    opacity: 0.6;
    margin: 0.3rem 0 0.2rem;
  }
  .group ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 28vh;
    overflow: auto;
  }
  .group li {
    padding: 0.12rem 0;
  }
  .group .empty {
    margin: 0.2rem 0;
    opacity: 0.55;
  }
  .group a.page-link {
    display: block;
    padding: 0.1rem 0.3rem;
    border-radius: 0.25rem;
    text-decoration: none;
    color: var(--link-color);
    border-bottom: 0;
  }
  .group a.page-link:hover {
    background: var(--code-bg);
  }
  .footer {
    margin-top: auto;
    border-top: 1px solid var(--guide-color);
    padding-top: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .search-trigger {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--code-bg);
    color: var(--fg);
    border: 1px solid var(--guide-color);
    border-radius: 0.3rem;
    padding: 0.3rem 0.5rem;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .search-trigger:hover {
    border-color: var(--link-color);
  }
  .search-trigger kbd {
    font-family: ui-monospace, monospace;
    font-size: 0.72rem;
    opacity: 0.7;
  }
  .error {
    color: #c33;
    font-size: 0.82rem;
  }

  /* Watcher status pill (D-40-05) — subtle connection indicator */
  .watcher-pill {
    display: flex;
    align-items: center;
    padding-top: 0.25rem;
  }
  .watcher-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .watcher-dot[data-status="connected"] {
    background: #22c55e; /* green */
  }
  .watcher-dot[data-status="reconnecting"] {
    background: #f59e0b; /* amber */
    animation: watcher-pulse 1.2s ease-in-out infinite;
  }
  .watcher-dot[data-status="offline"] {
    background: #9ca3af; /* grey */
  }
  @keyframes watcher-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: 0.4; transform: scale(0.75); }
  }
</style>
