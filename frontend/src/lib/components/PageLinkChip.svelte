<script lang="ts">
  // PageLinkChip is a small helper used by Sidebar (and could be used by any
  // other place that needs a `[[link]]`-style chip with unresolved styling).
  //
  // For inline block content we keep the existing markdown-it pathway —
  // those chips are emitted as raw HTML during render. The unresolved
  // styling for THOSE chips is applied retroactively in Block.svelte via a
  // post-render $effect that consults `sidebarPages`.
  //
  // This component is the Svelte equivalent for places where we build the
  // chip in template-space (Sidebar list, future BacklinksPanel source-page
  // headings, etc.).

  interface Props {
    name: string;
    resolved?: boolean;
    /** Override the inner label (e.g. journal `formattedTitle`). */
    label?: string;
  }
  let { name, resolved = true, label }: Props = $props();
</script>

<a
  class="page-link"
  class:unresolved={!resolved}
  href={`#/pages/${encodeURIComponent(name)}`}
  data-page={name}
>{label ?? name}</a>

<style>
  /* No-op: the rule lives in blocks.css so it applies uniformly to the
   * markdown-it-emitted chips inside <Block> content. We only re-declare
   * here as a comment for grep-discovery. */
</style>
