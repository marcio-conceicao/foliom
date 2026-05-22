<script lang="ts">
  import { push } from 'svelte-spa-router';
  import { renamePage } from '../api';
  import RenameModal from './RenameModal.svelte';

  interface Props {
    name: string;
    isJournal: boolean;
    formattedTitle: string | null;
    /** Number of backlinks to this page. Used to decide whether to show the
     *  confirmation modal before renaming. */
    backlinkCount?: number;
  }

  let { name, isJournal, formattedTitle, backlinkCount = 0 }: Props = $props();

  // Journal pages display the long-form formatted title ("May 21st, 2026")
  // produced by the backend (LNK-05). For ordinary pages we fall back to
  // the page name. The `name` (raw page-name / YYYY_MM_DD) is shown as a
  // subdued caption on journal entries so the URL <-> title relation is
  // visible to the user.
  const displayTitle = $derived(formattedTitle ?? name);

  // ─── Inline rename state (D-30-02) ───────────────────────────────────────

  let editing = $state(false);
  let draft = $state('');
  let error = $state('');
  let modalOpen = $state(false);

  function startEditing() {
    // Journal titles are not user-editable (D-30-02).
    if (isJournal) return;
    editing = true;
    draft = name;
    error = '';
  }

  function cancelEditing() {
    editing = false;
    error = '';
    modalOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      confirm();
    } else if (e.key === 'Escape') {
      cancelEditing();
    }
  }

  async function confirm() {
    const trimmed = draft.trim();
    if (!trimmed || trimmed === name) {
      cancelEditing();
      return;
    }
    if (backlinkCount > 0) {
      // Show modal to ask whether to rewrite backlinks.
      modalOpen = true;
    } else {
      await doRename(true);
    }
  }

  async function doRename(rewriteAll: boolean) {
    modalOpen = false;
    error = '';
    const trimmed = draft.trim();
    try {
      await renamePage(name, trimmed, rewriteAll);
      editing = false;
      // Navigate to the new page name.
      push('/pages/' + encodeURIComponent(trimmed));
    } catch (err: unknown) {
      const status = (err as { status?: number }).status;
      if (status === 409) {
        error = `A page named "${trimmed}" already exists.`;
      } else if (status === 400) {
        error = 'Name contains invalid characters.';
      } else {
        error = 'Rename failed. Please try again.';
      }
    }
  }
</script>

<header class="page-header">
  {#if editing}
    <div class="rename-row">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="rename-input"
        type="text"
        bind:value={draft}
        autofocus
        onkeydown={handleKeydown}
        onblur={cancelEditing}
      />
      {#if error}
        <span class="rename-error">{error}</span>
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <h1
      class:editable={!isJournal}
      onclick={startEditing}
    >{displayTitle}</h1>
  {/if}

  {#if isJournal && formattedTitle && name !== formattedTitle}
    <p class="caption">{name}</p>
  {/if}
</header>

{#if modalOpen}
  <RenameModal
    oldName={name}
    newName={draft}
    {backlinkCount}
    onRewriteAll={() => void doRename(true)}
    onRenameOnly={() => void doRename(false)}
    onCancel={cancelEditing}
  />
{/if}

<style>
  .page-header {
    margin: 0 0 1rem 0;
  }

  .page-header h1 {
    margin: 0;
    font-size: 1.5rem;
    line-height: 1.2;
  }

  .page-header h1.editable {
    cursor: text;
  }

  .page-header h1.editable:hover {
    opacity: 0.85;
  }

  .rename-row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .rename-input {
    font-size: 1.5rem;
    font-weight: bold;
    background: var(--bg-secondary, #2d2d2d);
    color: var(--fg, #ccc);
    border: 1px solid var(--accent, #5c9ef9);
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
    width: 100%;
    box-sizing: border-box;
    outline: none;
  }

  .rename-error {
    font-size: 0.8rem;
    color: var(--error, #e06c75);
  }

  .caption {
    margin: 0.25rem 0 0 0;
    color: var(--fg);
    opacity: 0.6;
    font-size: 0.85rem;
  }
</style>
