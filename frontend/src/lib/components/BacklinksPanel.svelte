<script lang="ts">
  // LNK-03: collapsible Backlinks panel rendered beneath the main page
  // content. Fetches /api/pages/<name>/backlinks on mount and whenever
  // `name` changes; groups by source page; renders each entry as a link
  // to /pages/<source>#block=<id> so the zoom-and-scroll logic (LNK-07,
  // shipped by 02-04) highlights the referencing block on click.
  //
  // Snippet text is NEVER rendered as HTML — backend strings flow through
  // Svelte's text-binding (`{...}`) which escapes by default. This
  // mitigates T-02-17 (XSS via backlink snippet) from the threat model.

  import { fetchBacklinks, type Backlink } from '../api';
  import { stripForRender } from '../markdown/strip';

  interface Props {
    name: string;
  }
  let { name }: Props = $props();

  let backlinks = $state<Backlink[] | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const current = name;
    backlinks = null;
    error = null;
    fetchBacklinks(current)
      .then((b) => {
        // Guard against late-arriving responses for a previous `name`.
        if (current === name) backlinks = b;
      })
      .catch((e: unknown) => {
        if (current === name) error = e instanceof Error ? e.message : String(e);
      });
  });

  function groupBy<T, K>(items: T[], key: (t: T) => K): Map<K, T[]> {
    const out = new Map<K, T[]>();
    for (const item of items) {
      const k = key(item);
      const bucket = out.get(k);
      if (bucket) bucket.push(item);
      else out.set(k, [item]);
    }
    return out;
  }

  // For the snippet preview we strip the segmenter prefix (leading TABs,
  // `- ` bullet, continuation `  `) so the user sees the actual text. We
  // pass depth=0 + empty properties/drawers because the backlink record
  // only carries the truncated text — drawer/property stripping has
  // already happened upstream (the backlink snippet column is built from
  // the indexer's display text).
  function previewOf(snippet: string): string {
    return stripForRender(snippet, 0, [], []).replace(/\r?\n$/, '').trim();
  }

  const grouped = $derived(
    backlinks ? groupBy(backlinks, (b) => b.page) : new Map<string, Backlink[]>(),
  );
  const groupEntries = $derived(Array.from(grouped.entries()));
  const total = $derived(backlinks?.length ?? 0);
</script>

<details class="backlinks" open>
  <summary>Backlinks ({total})</summary>

  {#if error}
    <p class="error">Erro ao carregar backlinks: {error}</p>
  {:else if backlinks === null}
    <p class="loading">Carregando…</p>
  {:else if backlinks.length === 0}
    <p class="empty">Sem backlinks.</p>
  {:else}
    {#each groupEntries as [sourcePage, entries] (sourcePage)}
      <section class="group">
        <h3>{sourcePage}</h3>
        <ul>
          {#each entries as bl (bl.blockId)}
            <li>
              <a
                href={`#/pages/${encodeURIComponent(sourcePage)}#block=${bl.blockId}`}
                data-page={sourcePage}
                data-block-id={bl.blockId}
              >{previewOf(bl.snippet)}</a>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  {/if}
</details>

<style>
  .backlinks {
    margin-top: 2rem;
    border-top: 1px solid var(--guide-color);
    padding-top: 0.75rem;
  }
  .backlinks > summary {
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg);
    opacity: 0.7;
    margin-bottom: 0.5rem;
  }
  .backlinks > summary:hover {
    opacity: 1;
  }
  .group {
    margin: 0.5rem 0;
  }
  .group h3 {
    font-size: 0.92rem;
    margin: 0.4rem 0 0.2rem;
    color: var(--link-color);
  }
  .group ul {
    list-style: none;
    margin: 0;
    padding: 0 0 0 0.5rem;
  }
  .group li {
    padding: 0.1rem 0;
  }
  .group a {
    text-decoration: none;
    color: var(--fg);
    display: block;
    padding: 0.15rem 0.3rem;
    border-radius: 0.25rem;
  }
  .group a:hover {
    background: var(--code-bg);
  }
  .empty,
  .loading {
    opacity: 0.55;
    font-size: 0.88rem;
    margin: 0.3rem 0;
  }
  .error {
    color: #c33;
    font-size: 0.88rem;
  }
</style>
