<script lang="ts">
  // SearchView — `/#/search?q=…&kind=…` deep-link route. Reuses the
  // palette body in `mode='inline'` so shared URLs render the same UX
  // as the modal palette, sans backdrop and centering chrome.
  //
  // svelte-spa-router strips the route hash before delegating; we read
  // `window.location.hash` directly so we can recover the `?q=` and
  // `?kind=` query string the router doesn't expose for hash routes.

  import { searchPalette } from '../stores';
  import SearchPalette from '../components/SearchPalette.svelte';

  function readQuery(): string {
    const hash = window.location.hash;
    const queryIndex = hash.indexOf('?');
    if (queryIndex === -1) return '';
    const params = new URLSearchParams(hash.slice(queryIndex + 1));
    return params.get('q') ?? '';
  }

  // Pre-populate the shared `searchPalette` store so the inline palette
  // body shows the deep-linked query without re-typing.
  $effect(() => {
    const q = readQuery();
    searchPalette.update((s) => ({ ...s, query: q }));
  });
</script>

<section class="search-view">
  <h1>Busca</h1>
  <SearchPalette mode="inline" />
</section>

<style>
  .search-view {
    max-width: 720px;
    margin: 0 auto;
  }
  .search-view h1 {
    font-size: 1.2rem;
    margin: 0 0 0.6rem;
    opacity: 0.85;
  }
</style>
