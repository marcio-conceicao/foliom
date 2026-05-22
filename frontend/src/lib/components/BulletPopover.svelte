<script lang="ts">
  import { tick } from 'svelte';
  import type { Block } from '../api';

  // Actions supported by the popover (EDT-12, D-30-04).
  export type PopoverAction =
    | 'cut'
    | 'copy'
    | 'duplicate'
    | 'fold'
    | 'zoom'
    | 'copy-as-md';

  type Props = {
    block: Block;
    onClose: () => void;
    onAction: (action: PopoverAction) => void;
  };

  let { block, onClose, onAction }: Props = $props();

  // The 6 menu items per D-30-04.
  const menuItems: Array<{ action: PopoverAction; label: string }> = [
    { action: 'cut',       label: 'Cut block' },
    { action: 'copy',      label: 'Copy block' },
    { action: 'duplicate', label: 'Duplicate' },
    { action: 'fold',      label: 'Fold' },
    { action: 'zoom',      label: 'Zoom' },
    { action: 'copy-as-md', label: 'Copy as markdown' },
  ];

  function handleItemClick(action: PopoverAction): void {
    onAction(action);
    onClose();
  }

  // Close on Escape key and click-outside.
  // Use $effect for lifecycle-safe listener registration/cleanup.
  $effect(() => {
    function onKeydown(e: KeyboardEvent): void {
      if (e.key === 'Escape') {
        onClose();
      }
    }

    function onClickOutside(e: MouseEvent): void {
      // Allow a tick for the event to settle before checking containment.
      const popoverEl = document.querySelector('.bullet-popover');
      if (popoverEl && !popoverEl.contains(e.target as Node)) {
        onClose();
      }
    }

    document.addEventListener('keydown', onKeydown);
    // Use mousedown so we close before the blur on the block element fires.
    document.addEventListener('mousedown', onClickOutside);

    return () => {
      document.removeEventListener('keydown', onKeydown);
      document.removeEventListener('mousedown', onClickOutside);
    };
  });
</script>

<!-- Absolute positioned menu relative to .block (D-30-04 + 03-RESEARCH §Bullet popover positioning).
     position: absolute; left: 100%; top: 0 — appears to the right of the block. -->
<menu
  class="bullet-popover"
  role="menu"
  aria-label="Block actions"
>
  {#each menuItems as item}
    <li role="none">
      <button
        type="button"
        role="menuitem"
        data-action={item.action}
        onclick={() => handleItemClick(item.action)}
      >
        {item.label}
      </button>
    </li>
  {/each}
</menu>

<style>
  .bullet-popover {
    position: absolute;
    left: 100%;
    top: 0;
    z-index: 100;
    margin: 0;
    padding: 4px 0;
    list-style: none;
    background: var(--color-bg, #fff);
    border: 1px solid var(--color-border, #e0e0e0);
    border-radius: 4px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    min-width: 160px;
    white-space: nowrap;
  }

  .bullet-popover li {
    display: block;
  }

  .bullet-popover button {
    display: block;
    width: 100%;
    padding: 6px 14px;
    text-align: left;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.875rem;
    color: var(--color-text, #222);
    line-height: 1.4;
  }

  .bullet-popover button:hover,
  .bullet-popover button:focus {
    background: var(--color-hover, #f0f0f0);
    outline: none;
  }
</style>
