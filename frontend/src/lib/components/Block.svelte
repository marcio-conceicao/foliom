<script lang="ts">
  import { tick } from 'svelte';
  import { push } from 'svelte-spa-router';
  import { md } from '../markdown';
  import { stripForRender } from '../markdown/strip';
  import type { Block as BlockData, MutationResponse, StaleConflict } from '../api';
  import { putBlock, deleteBlock, postBlock, patchBlockStructure, createPage } from '../api';
  import { sidebarPages } from '../stores';
  import { currentlyEditing } from '../stores/editing';
  import { treeOpLog } from '../stores/treeOpLog';
  import { BlockEditor, trySaveBlock } from '../editor/view';
  import { completionSource } from '../editor/autocomplete';
  import { detectBulletTree } from '../editor/paste';
  import { serializeBlockTree } from '../editor/serialize';
  import BulletPopover from './BulletPopover.svelte';
  import type { PopoverAction } from './BulletPopover.svelte';
  import Self from './Block.svelte';

  // Props extend BlockData with fileHash (plan 03-03 wire contract)
  // and optional callbacks to PageView for sibling creation and conflict handling.
  type Props = BlockData & {
    fileHash?: string;
    onSiblingCreate?: (afterBlockId: number, depth: number, prevHash: string) => void;
    onStaleConflict?: (serverHash: string) => void;
    onBlockDeleted?: (blockId: number, prevHash: string) => void;
    onBlockSaved?: (response: MutationResponse) => void;
    /** EDT-06: Merge current block into the previous block. */
    onMerge?: (currentBlockId: number, currentRaw: string, currentFileHash: string) => void;
    /** EDT-07: Move editing focus to the adjacent block ('up' = previous, 'down' = next). */
    onNavigate?: (direction: 'up' | 'down', currentBlockId: number) => void;
  };

  let {
    id,
    depth,
    raw,
    properties,
    drawers,
    children,
    fileHash = '',
    onSiblingCreate,
    onStaleConflict,
    onBlockDeleted,
    onBlockSaved,
    onMerge,
    onNavigate,
  }: Props = $props();

  // D-34: fold state is UI-only — no fetch, no persistence in Phase 2.
  let folded = $state(false);

  // D-30-04: bullet click popover state.
  let popoverOpen = $state(false);

  // Edit mode state (EDT-01)
  let editing = $state(false);
  let editorMountEl: HTMLDivElement | undefined = $state();
  const blockEditor = new BlockEditor();

  // Track current raw for edit session (may differ from prop if parent hasn't re-rendered yet).
  // Use a closure-captured reference to the latest prop value.
  // svelte-ignore state_referenced_locally
  let currentRaw = $state(raw);
  $effect(() => {
    // Keep currentRaw in sync when the prop changes (e.g., after mutation response)
    currentRaw = raw;
  });

  // Watch currentlyEditing store to enforce EDT-01 (single-block edit at a time).
  // When another block takes over editing, unmount our CM6 editor and save.
  currentlyEditing.subscribe((activeId) => {
    if (editing && activeId !== id) {
      saveAndUnmount();
    }
  });

  async function saveAndUnmount(): Promise<void> {
    if (!editing) return;
    // IME guard: trySaveBlock checks view.composing
    const saveState = trySaveBlock(blockEditor);
    if (saveState === 'saved') {
      const docText = blockEditor.readDocSafe();
      if (docText !== null && docText !== currentRaw) {
        await persistBlock(docText);
      }
    }
    // Destroy the view regardless of save result
    const finalDoc = blockEditor.unmount();
    if (finalDoc !== null) {
      currentRaw = finalDoc;
    }
    editing = false;
    editorMountEl = undefined;
  }

  async function persistBlock(docText: string): Promise<void> {
    const result = await putBlock(id, docText, fileHash);
    if ('stale' in result) {
      // T-03-11: 409 stale conflict → surface Reload banner via callback
      onStaleConflict?.(result.currentFileHash);
    } else {
      // Update local state from MutationResponse (no follow-up GET needed)
      currentRaw = docText;
      onBlockSaved?.(result);
    }
  }

  // Mount CM6 editor once the mount element is available in the DOM.
  $effect(() => {
    if (editing && editorMountEl && !blockEditor.view) {
      blockEditor.mount(editorMountEl, currentRaw, {
        onBoundary: handleBoundaryKey,
        onSave: (docText: string) => {
          // Called by boundary key handlers after unmount
          void persistBlock(docText);
        },
        completions: completionSource, // EDT-09: real [[link]] and #tag completions (plan 03-05)
        onPaste: handlePaste,
      });
    }
  });

  function handleBoundaryKey(key: import('../editor/boundary').BoundaryKey, view: import('@codemirror/view').EditorView): boolean {
    const doc = view.state.doc;
    const sel = view.state.selection.main;

    switch (key) {
      case 'Enter': {
        // EDT-04: Save current block + create sibling.
        // trySaveBlock gates IME (EDT-13).
        if (trySaveBlock(blockEditor) === 'skipped-due-to-ime') return true;
        const docText = blockEditor.readDocSafe();
        if (docText !== null) {
          // Unmount first, then signal PageView to create sibling.
          blockEditor.unmount();
          editing = false;
          // Push sibling creation to the log (tree op placeholder)
          treeOpLog.push({
            kind: 'Split',
            blockId: id,
            atOffset: sel.head,
            newBlockId: -1, // resolved after POST response in plan 03-05
          });
          // Persist + create sibling via callback
          void persistBlock(docText).then(() => {
            onSiblingCreate?.(id, depth, fileHash);
          });
        }
        return true;
      }

      case 'ShiftEnter':
        // Insert literal newline in block — let CM6 default handle it.
        return false;

      case 'Tab': {
        // EDT-05: Indent block. Push TreeOp, call PATCH /structure.
        treeOpLog.push({ kind: 'Indent', blockId: id, prevDepth: depth });
        // Save current edit, signal indent via callback
        void saveAndUnmount().then(() => {
          void patchBlockStructure(id, { op: 'indent', prevHash: fileHash }).then((r) => {
            if ('stale' in r) { onStaleConflict?.(r.currentFileHash); }
            else { onBlockSaved?.(r); }
          });
        });
        return true;
      }

      case 'ShiftTab': {
        // EDT-05: Outdent block.
        treeOpLog.push({ kind: 'Outdent', blockId: id, prevDepth: depth });
        void saveAndUnmount().then(() => {
          void patchBlockStructure(id, { op: 'outdent', prevHash: fileHash }).then((r) => {
            if ('stale' in r) { onStaleConflict?.(r.currentFileHash); }
            else { onBlockSaved?.(r); }
          });
        });
        return true;
      }

      case 'Backspace': {
        if (doc.length === 0) {
          // D-30-08: Empty block → Delete TreeOp
          treeOpLog.push({
            kind: 'Delete',
            blockId: id,
            snapshot: { raw: currentRaw, depth, parentId: null, ord: 0 },
          });
          blockEditor.unmount();
          editing = false;
          onBlockDeleted?.(id, fileHash);
          return true;
        }
        if (sel.head === 0 && doc.length > 0) {
          // EDT-06: Backspace at start of non-empty → Merge with previous block.
          // Save current text, unmount, then signal PageView to perform the merge.
          const docText = blockEditor.readDocSafe();
          const rawToMerge = docText !== null ? docText : currentRaw;
          blockEditor.unmount();
          editing = false;
          onMerge?.(id, rawToMerge, fileHash);
          return true;
        }
        // Mid-content: let CM6 default char-delete
        return false;
      }

      case 'ArrowUp': {
        // EDT-07: Navigate to previous block when at first line.
        const firstLine = doc.lineAt(0);
        if (sel.head <= firstLine.to) {
          void saveAndUnmount().then(() => {
            onNavigate?.('up', id);
          });
          return true;
        }
        return false;
      }

      case 'ArrowDown': {
        // EDT-07: Navigate to next block when at last line.
        const lastLine = doc.line(doc.lines);
        if (sel.head >= lastLine.from) {
          void saveAndUnmount().then(() => {
            onNavigate?.('down', id);
          });
          return true;
        }
        return false;
      }

      default:
        return false;
    }
  }

  // D-30-07: Paste handler — detect bullet hierarchy and insert as blocks.
  // Called by the CM6 domEventHandlers paste extension.
  // Returns true if handled (suppresses CM6 default insert), false for raw text paste.
  async function handlePaste(clipboardText: string): Promise<boolean> {
    const tree = detectBulletTree(clipboardText);
    if (!tree) return false; // Let CM6 handle plain text paste.

    // Insert each item via postBlock at appropriate depth after the current block.
    // We signal via the same onSiblingCreate callback that Enter uses.
    for (const item of tree.items) {
      onSiblingCreate?.(id, item.depth, fileHash);
    }
    return true;
  }

  // D-30-04: bullet left-click opens the popover. Right-click stays native.
  function handleBulletClick(event: MouseEvent): void {
    // Only respond to left-click (button === 0).
    if (event.button !== 0) return;
    // Don't open popover while in edit mode — bullet is inside the editor area.
    if (editing) return;
    event.stopPropagation();
    popoverOpen = true;
  }

  async function handlePopoverAction(action: PopoverAction): Promise<void> {
    switch (action) {
      case 'copy':
      case 'copy-as-md': {
        const text = serializeBlockTree({ id, depth, raw: currentRaw, properties, drawers, children });
        await navigator.clipboard.writeText(text).catch(() => {
          // clipboard API may be unavailable in some contexts — silently ignore
        });
        break;
      }

      case 'cut': {
        // Copy first, then delete.
        const text = serializeBlockTree({ id, depth, raw: currentRaw, properties, drawers, children });
        await navigator.clipboard.writeText(text).catch(() => {});
        treeOpLog.push({
          kind: 'Delete',
          blockId: id,
          snapshot: { raw: currentRaw, depth, parentId: null, ord: 0 },
        });
        onBlockDeleted?.(id, fileHash);
        break;
      }

      case 'duplicate': {
        const text = serializeBlockTree({ id, depth, raw: currentRaw, properties, drawers, children });
        const tree = detectBulletTree(text);
        if (tree) {
          // We need pageId from the parent — get it from fileHash context (plan 03-03).
          // Use ord-based insertion after the current block.
          // Since pageId is not directly available in Block.svelte, signal via onSiblingCreate.
          // For now: duplicate creates siblings via the same callback as Enter.
          for (const item of tree.items) {
            onSiblingCreate?.(id, item.depth, fileHash);
          }
        }
        break;
      }

      case 'fold': {
        folded = !folded;
        break;
      }

      case 'zoom': {
        // Navigate to this block's anchor (Phase 2 plan 02-04 zoom hook).
        // Get the current page name from the URL (svelte-spa-router path).
        const hash = window.location.hash;
        const pageMatch = hash.match(/#\/pages\/([^/?#]+)/);
        const pageName = pageMatch ? pageMatch[1] : '';
        push('/pages/' + pageName + '#block=' + id);
        break;
      }
    }
    popoverOpen = false;
  }

  const display = $derived(stripForRender(currentRaw, depth, properties, drawers));
  const rendered = $derived(display ? md.render(display) : '');

  // 02-05: retroactive `.unresolved` styling for `[[link]]` chips inside
  // rendered block HTML.
  let contentEl: HTMLDivElement | undefined = $state();
  let resolvedSet = $state<Set<string>>(new Set());
  sidebarPages.subscribe((pages) => {
    resolvedSet = new Set(pages.filter((p) => p.isResolved).map((p) => p.name));
  });
  $effect(() => {
    if (!contentEl) return;
    void rendered;
    void resolvedSet;
    if (resolvedSet.size === 0) {
      contentEl
        .querySelectorAll('a.page-link.unresolved')
        .forEach((el) => el.classList.remove('unresolved'));
      return;
    }
    contentEl.querySelectorAll<HTMLAnchorElement>('a.page-link[data-page]').forEach((el) => {
      const target = el.dataset.page ?? '';
      if (resolvedSet.has(target)) {
        el.classList.remove('unresolved');
      } else {
        el.classList.add('unresolved');
      }
    });
  });

  // depth === -1 is the page prelude: render children only, no chrome.
  const isPrelude = $derived(depth === -1);

  // Delegated click handler on .content — translates chip clicks into
  // svelte-spa-router navigations. Also handles click-to-edit.
  function handleContentClick(event: MouseEvent): void {
    const t = event.target as HTMLElement | null;
    if (!t) return;

    // Chip clicks take priority — don't enter edit mode.
    // D-30-03: unresolved-link click silently creates the page and navigates.
    const pageEl = t.closest('[data-page]') as HTMLElement | null;
    if (pageEl) {
      event.preventDefault();
      const target = pageEl.dataset.page ?? '';
      if (pageEl.classList.contains('unresolved')) {
        // LNK-04: create the page silently then navigate.
        createPage(target).catch(() => {
          // If create fails (e.g. already exists from a race), still navigate.
        }).finally(() => {
          push('/pages/' + encodeURIComponent(target));
        });
      } else {
        push('/pages/' + encodeURIComponent(target));
      }
      return;
    }
    const tagEl = t.closest('[data-tag]') as HTMLElement | null;
    if (tagEl) {
      event.preventDefault();
      const tag = tagEl.dataset.tag ?? '';
      push('/search?q=' + encodeURIComponent('#' + tag) + '&kind=tag');
      return;
    }

    // Click on block content → enter edit mode (EDT-01, EDT-02)
    if (!editing) {
      // Signal EDT-01: this block takes over editing, others will unmount
      currentlyEditing.set(id);
      editing = true;
    }
  }

  // Blur handler: save when CM6 loses focus (D-30-01)
  function handleBlur(event: FocusEvent): void {
    // Check if focus is still within our block element (e.g., clicked another CM6 element)
    const relatedTarget = event.relatedTarget as Node | null;
    const blockEl = (event.currentTarget as HTMLElement).closest('.block');
    if (blockEl && relatedTarget && blockEl.contains(relatedTarget)) {
      return; // Focus stayed within the block
    }
    void saveAndUnmount();
  }
</script>

{#if isPrelude}
  {#each children as child (child.id)}
    <Self {...child} {fileHash} {onSiblingCreate} {onStaleConflict} {onBlockDeleted} {onBlockSaved} {onMerge} {onNavigate} />
  {/each}
{:else}
  <div class="block" class:editing data-block-id={id} data-depth={depth} id={`block-${id}`}>
    <button
      type="button"
      class="fold-toggle"
      class:has-children={children.length > 0}
      aria-label={folded ? 'Expand' : 'Collapse'}
      onclick={(e) => {
        // Left-click on bullet when NOT editing → open popover (D-30-04).
        // The fold toggle retains its function when popover is dismissed.
        if (!editing && e.button === 0) {
          e.stopPropagation();
          popoverOpen = !popoverOpen;
        } else {
          folded = !folded;
        }
      }}
    >
      <span class="bullet">{folded ? '▶' : '•'}</span>
    </button>

    {#if popoverOpen}
      <BulletPopover
        block={{ id, depth, raw: currentRaw, properties, drawers, children }}
        onClose={() => (popoverOpen = false)}
        onAction={handlePopoverAction}
      />
    {/if}

    {#if editing}
      <!-- CM6 editor mount point. The $effect above mounts BlockEditor here. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="editor-mount"
        bind:this={editorMountEl}
        onblur={handleBlur}
      ></div>
    {:else}
      <div
        class="content"
        role="presentation"
        bind:this={contentEl}
        onclick={handleContentClick}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            const tgt = e.target as HTMLElement | null;
            if (tgt && (tgt.closest('[data-page]') || tgt.closest('[data-tag]'))) {
              handleContentClick(e as unknown as MouseEvent);
            }
          }
        }}
      >
        {@html rendered}
      </div>
    {/if}
  </div>

  {#if !folded && children.length > 0}
    <div class="children" style:--depth={depth + 1}>
      {#each children as child (child.id)}
        <Self {...child} {fileHash} {onSiblingCreate} {onStaleConflict} {onBlockDeleted} {onBlockSaved} {onMerge} {onNavigate} />
      {/each}
    </div>
  {/if}
{/if}
