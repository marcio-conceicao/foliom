<script lang="ts">
  // SearchPalette — the SCH-01 / SCH-02 / SCH-03 / LNK-07 entry point.
  //
  // Two render modes share the same body:
  //   • `mode='modal'` (default) — full-screen backdrop, centered panel,
  //     click-outside closes. Mounted by App.svelte when the
  //     `searchPalette` store is open.
  //   • `mode='inline'` — no backdrop, no positioning. Used by the
  //     `#/search?q=…` route (SearchView.svelte) for shareable deep links.
  //
  // Query routing (all branches debounced at 150ms):
  //   • starts with `#`  → fetchSearch(query.slice(1), 'tag', 50)
  //   • starts with `[[` → fetchPageTitles() (cached) + client-side filter
  //   • otherwise        → fetchSearch(query, 'content', 50)
  //
  // Snippet rendering uses the `sanitize.ts` allow-list (T-02-20 — only
  // <mark>/</mark> survive). Enter on a row navigates to
  // `#/pages/<page>#block=<blockId>` and the zoom listener installed in
  // `lib/zoom.ts` picks it up.
  //
  // AbortController-based cancellation (T-02-22): each new debounced
  // run cancels the in-flight fetch from the previous one. Required so
  // a slow `/api/search` response doesn't overwrite results from a
  // newer query.

  import { push } from 'svelte-spa-router';
  import { fetchPageTitles, type SearchHit, type SearchKind } from '../api';
  import { searchPalette } from '../stores';
  import SearchResult from './SearchResult.svelte';

  type Mode = 'modal' | 'inline';
  let { mode = 'modal' as Mode }: { mode?: Mode } = $props();

  const DEBOUNCE_MS = 150;
  const RESULT_LIMIT = 50;

  // Local copy of the store query so the <input> can be controlled.
  // We forward changes back to the store so SearchView's URL-bound
  // initialization path works symmetrically.
  let query = $state<string>('');
  searchPalette.subscribe((s) => {
    if (s.query !== query) query = s.query;
  });

  let results = $state<SearchHit[]>([]);
  let cursor = $state<number>(0);
  let lastSubmittedQuery = $state<string>('');
  let didSearch = $state<boolean>(false);

  let debounceTimer: number | undefined;
  let inflight: AbortController | null = null;

  // Module-level cache for /api/page-titles — one fetch per session is
  // enough; titles change rarely and the [[ branch is forgiving of
  // staleness (UI shows whatever was last known).
  let pageTitlesPromise: Promise<string[]> | null = null;
  function getPageTitles(): Promise<string[]> {
    if (!pageTitlesPromise) pageTitlesPromise = fetchPageTitles();
    return pageTitlesPromise;
  }

  function scheduleSearch(raw: string): void {
    if (debounceTimer !== undefined) clearTimeout(debounceTimer);
    const trimmed = raw.trim();
    if (!trimmed) {
      // Empty / whitespace — clear without firing a request.
      if (inflight) {
        inflight.abort();
        inflight = null;
      }
      results = [];
      didSearch = false;
      cursor = 0;
      return;
    }
    debounceTimer = window.setTimeout(() => void runSearch(trimmed), DEBOUNCE_MS);
  }

  async function runSearch(q: string): Promise<void> {
    // Cancel any in-flight request so older responses can't clobber
    // newer ones (T-02-22 race mitigation).
    if (inflight) inflight.abort();
    inflight = new AbortController();
    const signal = inflight.signal;

    lastSubmittedQuery = q;

    try {
      if (q.startsWith('[[')) {
        const needle = q.slice(2).trim().toLowerCase();
        const titles = await getPageTitles();
        if (signal.aborted) return;
        const matches = needle
          ? titles.filter((t) => t.toLowerCase().includes(needle))
          : titles.slice(0, RESULT_LIMIT);
        results = matches.slice(0, RESULT_LIMIT).map((t) => ({
          page: t,
          // blockId 0 = "open page top"; the zoom-and-scroll listener
          // ignores zero block IDs (parseBlockFragment requires \d+).
          blockId: 0,
          snippet: t,
        }));
      } else {
        let kind: SearchKind;
        let needle: string;
        if (q.startsWith('#')) {
          kind = 'tag';
          needle = q.slice(1).trim();
        } else {
          kind = 'content';
          needle = q;
        }
        if (!needle) {
          results = [];
          didSearch = true;
          cursor = 0;
          return;
        }
        const params = new URLSearchParams({
          q: needle,
          kind,
          limit: String(RESULT_LIMIT),
        });
        const res = await fetch(`/api/search?${params.toString()}`, {
          headers: { Accept: 'application/json' },
          signal,
        });
        if (signal.aborted) return;
        if (!res.ok) {
          results = [];
          didSearch = true;
          cursor = 0;
          return;
        }
        results = (await res.json()) as SearchHit[];
      }
      didSearch = true;
      cursor = 0;
    } catch (err) {
      // AbortError is expected when a newer keystroke supersedes us.
      if ((err as Error | null)?.name === 'AbortError') return;
      results = [];
      didSearch = true;
      cursor = 0;
    }
  }

  function close(): void {
    if (mode === 'modal') searchPalette.set({ open: false, query: '' });
  }

  function open(hit: SearchHit): void {
    // Numeric coercion (T-02-21) — non-numeric blockIds get NaN and the
    // fragment is omitted, so the user just lands at the page top.
    const blockId = Number(hit.blockId);
    const target = `/pages/${encodeURIComponent(hit.page)}`;
    push(target);
    // svelte-spa-router treats `#` as its own route boundary; we layer
    // the foliom-specific `#block=` sub-fragment on top of the route
    // hash after the route has been pushed. The zoom listener installed
    // in main.ts picks up the hashchange.
    if (Number.isFinite(blockId) && blockId > 0) {
      requestAnimationFrame(() => {
        window.location.hash = `#/pages/${encodeURIComponent(hit.page)}#block=${blockId}`;
      });
    }
    close();
  }

  function onInput(e: Event): void {
    const value = (e.target as HTMLInputElement).value;
    query = value;
    // Mirror to the store so the inline (SearchView) and modal paths stay in sync.
    searchPalette.update((s) => ({ ...s, query: value }));
    scheduleSearch(value);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (results.length === 0) return;
      cursor = Math.min(cursor + 1, results.length - 1);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (results.length === 0) return;
      cursor = Math.max(cursor - 1, 0);
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const hit = results[cursor];
      if (hit) open(hit);
      return;
    }
    // Esc inside the palette input is owned here (the global keys.ts
    // handler defers to inputs). Close the modal; in inline mode it's
    // a no-op so the user can keep typing.
    if (e.key === 'Escape' && mode === 'modal') {
      e.preventDefault();
      close();
    }
  }

  function onBackdropClick(e: MouseEvent): void {
    // Only close when the click landed on the backdrop itself, not a child.
    if (e.target === e.currentTarget) close();
  }
</script>

{#if mode === 'modal'}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="palette-backdrop" onclick={onBackdropClick}>
    <div class="palette-panel" role="dialog" aria-label="Buscar">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        type="search"
        class="palette-input"
        placeholder="Buscar (Ctrl+K) — use # para tag, [[ para página"
        autocomplete="off"
        spellcheck="false"
        value={query}
        oninput={onInput}
        onkeydown={onKeydown}
        autofocus
        aria-label="Consulta de busca"
      />
      <ul class="palette-results" role="listbox">
        {#each results as hit, i (i + ':' + hit.page + ':' + hit.blockId)}
          <SearchResult
            page={hit.page}
            snippet={hit.snippet}
            active={i === cursor}
            onclick={() => open(hit)}
            onmouseenter={() => (cursor = i)}
          />
        {/each}
        {#if didSearch && results.length === 0}
          <li class="empty">Sem resultados para '{lastSubmittedQuery}'.</li>
        {/if}
      </ul>
    </div>
  </div>
{:else}
  <div class="palette-inline">
    <input
      type="search"
      class="palette-input"
      placeholder="Buscar — use # para tag, [[ para página"
      autocomplete="off"
      spellcheck="false"
      value={query}
      oninput={onInput}
      onkeydown={onKeydown}
      aria-label="Consulta de busca"
    />
    <ul class="palette-results" role="listbox">
      {#each results as hit, i (i + ':' + hit.page + ':' + hit.blockId)}
        <SearchResult
          page={hit.page}
          snippet={hit.snippet}
          active={i === cursor}
          onclick={() => open(hit)}
          onmouseenter={() => (cursor = i)}
        />
      {/each}
      {#if didSearch && results.length === 0}
        <li class="empty">Sem resultados para '{lastSubmittedQuery}'.</li>
      {/if}
    </ul>
  </div>
{/if}
