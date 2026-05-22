<script lang="ts">
  import { tick } from 'svelte';
  import { fetchPage, postBlock, deleteBlock, putBlock, type PageDetail, type MutationResponse, type Block as BlockData } from '../api';
  import { currentPage } from '../stores';
  import Block from '../components/Block.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import BacklinksPanel from '../components/BacklinksPanel.svelte';
  import { applyZoomFromHash } from '../zoom';
  import { treeOpLog } from '../stores/treeOpLog';
  import { currentlyEditing } from '../stores/editing';
  import { externalConflict } from '../stores/watcher';

  interface Params { name: string }
  let { params }: { params: Params } = $props();

  let detail = $state<PageDetail | null>(null);
  let error = $state<string | null>(null);

  // T-03-11: stale conflict banner state.
  // Set to true when any mutation returns 409 Conflict, OR when the watcher
  // detects an external edit while a block is being edited (SNC-06, D-40-04).
  let staleConflict = $state(false);

  // SNC-06: watcher-driven conflict path (Phase 4 plan 04-02).
  // Subscribes to externalConflict store set by watcher.ts on pages_updated.
  // - Block in edit mode: surface the existing staleConflict banner.
  // - No block in edit mode: silent reload (no banner needed).
  // Consumer must clear externalConflict after handling (T-04-06 mitigation).
  $effect(() => {
    const conflict = $externalConflict;
    if (conflict === null) return;
    if ($currentlyEditing !== null) {
      staleConflict = true;          // surface the Phase 3 banner
    } else {
      void reload();                 // silent reload — no editor active
    }
    externalConflict.set(null);      // consume so the banner doesn't persist
  });

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

  /**
   * Flatten the nested block tree to a depth-first ordered list.
   * Prelude blocks (depth === -1) are expanded to their children in-place.
   */
  function flattenBlocks(blocks: BlockData[]): BlockData[] {
    const out: BlockData[] = [];
    function walk(bs: BlockData[]): void {
      for (const b of bs) {
        if (b.depth === -1) {
          walk(b.children);
        } else {
          out.push(b);
          walk(b.children);
        }
      }
    }
    walk(blocks);
    return out;
  }

  /**
   * EDT-06: Merge currentBlockId into the preceding block.
   * Sequence:
   *   1. DELETE /api/blocks/:currentBlockId (removes from file)
   *   2. PUT  /api/blocks/:prevId { raw: prevRaw + currentRaw, prevHash }
   *   3. Push a Merge TreeOp with full snapshot for undo
   *   4. Update detail from final MutationResponse
   */
  async function handleMerge(currentBlockId: number, currentRaw: string, currentFileHash: string): Promise<void> {
    if (!detail) return;

    const flat = flattenBlocks(detail.blocks);
    const currentIdx = flat.findIndex((b) => b.id === currentBlockId);
    if (currentIdx <= 0) {
      // No previous block — nothing to merge into (first block in page).
      return;
    }
    const prevBlock = flat[currentIdx - 1];
    const prevRaw = prevBlock.raw;

    // Step 1: Delete the current block from the file.
    const deleteResult = await deleteBlock(currentBlockId, currentFileHash);
    if ('stale' in deleteResult) {
      handleStaleConflict(deleteResult.currentFileHash);
      return;
    }
    // deleteBlock returns 204 → { blockSubtree: [], fileHash: '' } or a MutationResponse.
    // Update detail if we got a real subtree; always update fileHash.
    const newHash = deleteResult.fileHash || detail.fileHash || '';
    if (deleteResult.blockSubtree.length > 0) {
      detail = { ...detail, fileHash: newHash, blocks: mergeBlockSubtree(detail.blocks, deleteResult.blockSubtree) };
    } else {
      detail = { ...detail, fileHash: newHash };
    }
    currentPage.set(detail);

    // Step 2: Update prevBlock with the concatenated raw text.
    const mergedRaw = prevRaw + currentRaw;
    const putResult = await putBlock(prevBlock.id, mergedRaw, detail.fileHash ?? '');
    if ('stale' in putResult) {
      handleStaleConflict(putResult.currentFileHash);
      return;
    }

    // Step 3: Push the Merge TreeOp with full undo snapshot.
    treeOpLog.push({
      kind: 'Merge',
      blockId: currentBlockId,
      mergedIntoId: prevBlock.id,
      originalRaw: currentRaw,
      prevOriginalRaw: prevRaw,
    });

    // Step 4: Apply MutationResponse from the PUT.
    handleBlockSaved(putResult);

    // Focus the previous block after the merge (place cursor at end of prevRaw boundary).
    await tick();
    currentlyEditing.set(prevBlock.id);
  }

  /**
   * EDT-07: Focus the adjacent block when ArrowUp/Down is pressed at an edge.
   * PageView knows the flat block ordering; it sets currentlyEditing to the adjacent block.
   */
  function handleNavigate(direction: 'up' | 'down', currentBlockId: number): void {
    if (!detail) return;

    const flat = flattenBlocks(detail.blocks);
    const currentIdx = flat.findIndex((b) => b.id === currentBlockId);
    if (currentIdx < 0) return;

    const targetIdx = direction === 'up' ? currentIdx - 1 : currentIdx + 1;
    if (targetIdx < 0 || targetIdx >= flat.length) return;

    const targetBlock = flat[targetIdx];
    currentlyEditing.set(targetBlock.id);
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
        onMerge={handleMerge}
        onNavigate={handleNavigate}
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
