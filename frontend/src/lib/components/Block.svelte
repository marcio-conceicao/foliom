<script lang="ts">
  import { push } from 'svelte-spa-router';
  import { md } from '../markdown';
  import { stripForRender } from '../markdown/strip';
  import type { Block as BlockData } from '../api';
  import Self from './Block.svelte';

  type Props = BlockData;

  let { id, depth, raw, properties, drawers, children }: Props = $props();

  // D-34: fold state is UI-only — no fetch, no persistence in Phase 2.
  let folded = $state(false);

  const display = $derived(stripForRender(raw, depth, properties, drawers));
  const rendered = $derived(display ? md.render(display) : '');

  // depth === -1 is the page prelude: render children only, no chrome.
  const isPrelude = $derived(depth === -1);

  // Delegated click handler on .content — translates chip clicks into
  // svelte-spa-router navigations. Tag chips go to the search view filtered
  // by `kind=tag`; page chips navigate to /pages/<name>. This is the
  // resolution of 02-RESEARCH Open Question 3 — no dedicated tag-page view
  // exists in v1; search-by-tag is the user-facing destination.
  function handleContentClick(event: MouseEvent): void {
    const t = event.target as HTMLElement | null;
    if (!t) return;
    const pageEl = t.closest('[data-page]') as HTMLElement | null;
    if (pageEl) {
      event.preventDefault();
      const target = pageEl.dataset.page ?? '';
      push('/pages/' + encodeURIComponent(target));
      return;
    }
    const tagEl = t.closest('[data-tag]') as HTMLElement | null;
    if (tagEl) {
      event.preventDefault();
      const tag = tagEl.dataset.tag ?? '';
      // Search palette uses `q=#<tag>&kind=tag` per Phase 2 SCH-01/02.
      push('/search?q=' + encodeURIComponent('#' + tag) + '&kind=tag');
    }
  }
</script>

{#if isPrelude}
  {#each children as child (child.id)}
    <Self {...child} />
  {/each}
{:else}
  <div class="block" data-block-id={id} data-depth={depth} id={`block-${id}`}>
    <button
      type="button"
      class="fold-toggle"
      class:has-children={children.length > 0}
      aria-label={folded ? 'Expand' : 'Collapse'}
      onclick={() => (folded = !folded)}
    >
      <span class="bullet">{folded ? '▶' : '•'}</span>
    </button>

    <div
      class="content"
      role="presentation"
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
  </div>

  {#if !folded && children.length > 0}
    <div class="children" style:--depth={depth + 1}>
      {#each children as child (child.id)}
        <Self {...child} />
      {/each}
    </div>
  {/if}
{/if}
