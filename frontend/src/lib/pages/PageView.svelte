<script lang="ts">
  import { tick } from 'svelte';
  import { fetchPage, type PageDetail } from '../api';
  import { currentPage } from '../stores';
  import Block from '../components/Block.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import BacklinksPanel from '../components/BacklinksPanel.svelte';
  import { applyZoomFromHash } from '../zoom';

  interface Params { name: string }
  let { params }: { params: Params } = $props();

  let detail = $state<PageDetail | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const name = params.name;
    detail = null;
    error = null;
    fetchPage(name)
      .then(async (d) => {
        detail = d;
        currentPage.set(d);
        // After the block tree renders, honor any `#block=N` deep link.
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
    <BacklinksPanel name={detail.name} />
  {/if}
</section>

<style>
  .error { color: #c33; }
  .loading { opacity: 0.6; }
</style>
