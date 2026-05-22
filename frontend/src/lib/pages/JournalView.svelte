<script lang="ts">
  import { tick } from 'svelte';
  import { fetchPage, type PageDetail } from '../api';
  import { currentPage } from '../stores';
  import Block from '../components/Block.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import { applyZoomFromHash } from '../zoom';

  interface Params { date: string }
  let { params }: { params: Params } = $props();

  // The route param uses YYYY_MM_DD (Logseq journal page-name convention).
  // Accept YYYY-MM-DD too for friendlier URLs and normalize before fetch.
  function normalizeJournalName(input: string): string {
    return input.replaceAll('-', '_');
  }

  let detail = $state<PageDetail | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const pageName = normalizeJournalName(params.date);
    detail = null;
    error = null;
    fetchPage(pageName)
      .then(async (d) => {
        detail = d;
        currentPage.set(d);
        await tick();
        applyZoomFromHash();
      })
      .catch((e: unknown) => {
        error = e instanceof Error ? e.message : String(e);
      });
  });
</script>

<section class="page">
  {#if error}
    <div class="error">Erro ao carregar: {error}</div>
  {:else if detail === null}
    <div class="loading">Carregando…</div>
  {:else}
    <PageHeader
      name={detail.name}
      isJournal={detail.isJournal}
      formattedTitle={detail.formattedTitle}
    />
    {#each detail.blocks as block (block.id)}
      <Block {...block} />
    {/each}
  {/if}
</section>

<style>
  .error { color: #c33; }
  .loading { opacity: 0.6; }
</style>
