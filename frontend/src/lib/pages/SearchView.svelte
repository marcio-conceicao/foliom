<script lang="ts">
  import { fetchSearch, type SearchHit, type SearchKind } from '../api';

  let hits = $state<SearchHit[] | null>(null);
  let error = $state<string | null>(null);
  let queryParam = $state<string>('');

  function readQuery(): { q: string; kind: SearchKind | undefined } {
    // svelte-spa-router strips the hash; we read window.location.hash directly.
    // Format expected: `#/search?q=foo&kind=tag`
    const hash = window.location.hash;
    const queryIndex = hash.indexOf('?');
    if (queryIndex === -1) return { q: '', kind: undefined };
    const params = new URLSearchParams(hash.slice(queryIndex + 1));
    const kind = params.get('kind');
    return {
      q: params.get('q') ?? '',
      kind: kind === 'tag' || kind === 'content' ? kind : undefined,
    };
  }

  $effect(() => {
    const { q, kind } = readQuery();
    queryParam = q;
    hits = null;
    error = null;
    if (!q) {
      hits = [];
      return;
    }
    fetchSearch(q, kind)
      .then((h) => {
        hits = h;
      })
      .catch((e: unknown) => {
        error = e instanceof Error ? e.message : String(e);
      });
  });
</script>

<section>
  <h1>Busca</h1>
  <p>Consulta: <code>{queryParam || '(vazia)'}</code></p>
  {#if error}
    <p class="error">Erro: {error}</p>
  {:else if hits === null}
    <p>Carregando…</p>
  {:else}
    <pre>{JSON.stringify(hits, null, 2)}</pre>
  {/if}
</section>

<style>
  .error { color: #c33; }
  pre { font-family: ui-monospace, monospace; font-size: 0.85rem; overflow: auto; }
</style>
