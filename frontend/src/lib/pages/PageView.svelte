<script lang="ts">
  import { tick } from 'svelte';
  import { fetchPage, postBlock, deleteBlock, type PageDetail, type MutationResponse, type Block as BlockData } from '../api';
  import { currentPage } from '../stores';
  import Block from '../components/Block.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import BacklinksPanel from '../components/BacklinksPanel.svelte';
  import { applyZoomFromHash } from '../zoom';

  interface Params { name: string }
  let { params }: { params: Params } = $props();

  let detail = $state<PageDetail | null>(null);
  let error = $state<string | null>(null);

  // T-03-11: stale conflict banner state.
  // Set to true when any mutation returns 409 Conflict.
  let staleConflict = $state(false);

  $effect(() => {
    const name = params.name;
    detail = null;
    error = null;
    staleConflict = false;
    fetchPage(name)
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

  async function reload(): Promise<void> {
    if (!detail) return;
    staleConflict = false;
    try {
      const fresh = await fetchPage(detail.name);
      detail = fresh;
      currentPage.set(fresh);
      await tick();
      applyZoomFromHash();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function handleStaleConflict(_serverHash: string): void {
    // T-03-11: surface Reload banner; don't attempt to merge automatically.
    staleConflict = true;
  }

  /**
   * Merge MutationResponse.blockSubtree into detail.blocks (replace-by-id).
   * Called after a successful PUT/POST/PATCH/DELETE via the Block.svelte callback.
   */
  function handleBlockSaved(response: MutationResponse): void {
    if (!detail) return;
    // Update the file hash for subsequent mutations
    detail = { ...detail, fileHash: response.fileHash };
    // Replace the affected blocks in the tree from the response subtree.
    if (response.blockSubtree.length > 0) {
      detail = { ...detail, blocks: mergeBlockSubtree(detail.blocks, response.blockSubtree) };
    }
    currentPage.set(detail);
  }

  /**
   * Merge updatedBlocks into the existing tree by replacing matching root-level blocks.
   * For Phase 3 plan 03-04, the response subtree is the full page tree (from assemble_tree),
   * so we simply replace the entire blocks array.
   */
  function mergeBlockSubtree(existing: BlockData[], updated: BlockData[]): BlockData[] {
    // The mutation response blockSubtree is the full page block tree.
    // Replace entirely (plan 03-05 will do finer-grained merging if needed).
    if (updated.length > 0) return updated;
    return existing;
  }

  function handleSiblingCreate(afterBlockId: number, depth: number, prevHash: string): void {
    if (!detail || !detail.id) return;
    // Placeholder: full sibling creation wired in plan 03-05.
    // For now, create a sibling via the POST /api/blocks endpoint.
    void postBlock({
      pageId: detail.id,
      parentId: null,
      ord: 9999, // append at end — server computes real ord
      depth,
      raw: `${'\t'.repeat(depth)}- `,
      prevHash,
    }).then((result) => {
      if ('stale' in result) {
        handleStaleConflict(result.currentFileHash);
      } else {
        handleBlockSaved(result);
      }
    });
  }

  function handleBlockDeleted(blockId: number, prevHash: string): void {
    if (!detail) return;
    void deleteBlock(blockId, prevHash).then((result) => {
      if ('stale' in result) {
        handleStaleConflict(result.currentFileHash);
      } else {
        handleBlockSaved(result);
      }
    });
  }
</script>

<section class="page">
  {#if staleConflict}
    <div class="banner-stale" role="alert">
      External edit detected —
      <button type="button" onclick={reload}>Reload</button>
    </div>
  {/if}

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
      <Block
        {...block}
        fileHash={detail.fileHash ?? ''}
        onSiblingCreate={handleSiblingCreate}
        onStaleConflict={handleStaleConflict}
        onBlockDeleted={handleBlockDeleted}
        onBlockSaved={handleBlockSaved}
      />
    {/each}
    <BacklinksPanel name={detail.name} />
  {/if}
</section>

<style>
  .error { color: #c33; }
  .loading { opacity: 0.6; }
  .banner-stale {
    position: sticky;
    top: 0;
    z-index: 100;
    background: var(--color-warn, #fff3cd);
    border-bottom: 1px solid var(--color-warn-border, #ffc107);
    padding: 0.5rem 1rem;
    font-size: 0.9rem;
  }
  .banner-stale button {
    margin-left: 0.5rem;
    font-weight: 600;
    text-decoration: underline;
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
  }
</style>
