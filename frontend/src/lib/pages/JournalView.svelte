<script lang="ts">
  import { fetchPage, type PageDetail } from '../api';
  import { currentPage } from '../stores';

  interface Params { date: string }
  let { params }: { params: Params } = $props();

  let detail = $state<PageDetail | null>(null);
  let error = $state<string | null>(null);

  // The route param uses YYYY_MM_DD (Logseq journal page-name convention).
  // We accept YYYY-MM-DD too for friendlier URLs and normalize before fetch.
  function normalizeJournalName(input: string): string {
    return input.replaceAll('-', '_');
  }

  $effect(() => {
    const pageName = normalizeJournalName(params.date);
    detail = null;
    error = null;
    fetchPage(pageName)
      .then((d) => {
        detail = d;
        currentPage.set(d);
      })
      .catch((e: unknown) => {
        error = e instanceof Error ? e.message : String(e);
      });
  });
</script>

<section>
  <h1>Journal: {params.date}</h1>
  {#if error}
    <p class="error">Erro ao carregar: {error}</p>
  {:else if detail === null}
    <p>Carregando…</p>
  {:else}
    <pre>{JSON.stringify(detail, null, 2)}</pre>
  {/if}
</section>

<style>
  .error { color: #c33; }
  pre { font-family: ui-monospace, monospace; font-size: 0.85rem; overflow: auto; }
</style>
