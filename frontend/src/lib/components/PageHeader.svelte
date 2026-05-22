<script lang="ts">
  interface Props {
    name: string;
    isJournal: boolean;
    formattedTitle: string | null;
  }

  let { name, isJournal, formattedTitle }: Props = $props();

  // Journal pages display the long-form formatted title ("May 21st, 2026")
  // produced by the backend (LNK-05). For ordinary pages we fall back to
  // the page name. The `name` (raw page-name / YYYY_MM_DD) is shown as a
  // subdued caption on journal entries so the URL <-> title relation is
  // visible to the user.
  const displayTitle = $derived(formattedTitle ?? name);
</script>

<header class="page-header">
  <h1>{displayTitle}</h1>
  {#if isJournal && formattedTitle && name !== formattedTitle}
    <p class="caption">{name}</p>
  {/if}
</header>

<style>
  .page-header {
    margin: 0 0 1rem 0;
  }
  .page-header h1 {
    margin: 0;
    font-size: 1.5rem;
    line-height: 1.2;
  }
  .caption {
    margin: 0.25rem 0 0 0;
    color: var(--fg);
    opacity: 0.6;
    font-size: 0.85rem;
  }
</style>
