<script lang="ts">
  interface Props {
    oldName: string;
    newName: string;
    backlinkCount: number;
    onRewriteAll: () => void;
    onRenameOnly: () => void;
    onCancel: () => void;
  }

  let { oldName, newName, backlinkCount, onRewriteAll, onRenameOnly, onCancel }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onCancel();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onCancel();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="modal-backdrop"
  role="dialog"
  aria-modal="true"
  aria-label="Rename page"
  data-testid="rename-modal"
  onkeydown={handleKeydown}
  onclick={handleBackdropClick}
>
  <div class="modal">
    <p class="modal-message">
      Rewrite <strong>{backlinkCount}</strong> reference{backlinkCount !== 1 ? 's' : ''} to
      <code>[[{oldName}]]</code> across all files?
    </p>
    <div class="modal-actions">
      <button
        type="button"
        class="btn-primary"
        data-action="rewrite-all"
        autofocus
        onclick={onRewriteAll}
      >
        Rewrite all
      </button>
      <button
        type="button"
        class="btn-secondary"
        data-action="rename-only"
        onclick={onRenameOnly}
      >
        Rename without rewriting
      </button>
      <button
        type="button"
        class="btn-cancel"
        data-action="cancel"
        onclick={onCancel}
      >
        Cancel
      </button>
    </div>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: var(--bg, #1e1e1e);
    border: 1px solid var(--border, #333);
    border-radius: 6px;
    padding: 1.5rem;
    min-width: 320px;
    max-width: 480px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  .modal-message {
    margin: 0 0 1rem 0;
    line-height: 1.5;
    font-size: 0.95rem;
    color: var(--fg, #ccc);
  }

  .modal-message code {
    font-family: monospace;
    background: var(--code-bg, #2d2d2d);
    padding: 0.1em 0.3em;
    border-radius: 3px;
  }

  .modal-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  button {
    padding: 0.4rem 0.9rem;
    border-radius: 4px;
    border: 1px solid transparent;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .btn-primary {
    background: var(--accent, #5c9ef9);
    color: #fff;
    border-color: var(--accent, #5c9ef9);
  }

  .btn-primary:hover {
    filter: brightness(1.1);
  }

  .btn-secondary {
    background: var(--bg-secondary, #2d2d2d);
    color: var(--fg, #ccc);
    border-color: var(--border, #555);
  }

  .btn-cancel {
    background: transparent;
    color: var(--fg-muted, #888);
    border-color: transparent;
    margin-left: auto;
  }
</style>
