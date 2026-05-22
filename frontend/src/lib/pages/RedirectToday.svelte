<script lang="ts">
  import { replace } from 'svelte-spa-router';
  import { resolveJournalToday } from '../api';

  let error = $state<string | null>(null);

  $effect(() => {
    resolveJournalToday()
      .then((name) => {
        replace(`/journals/${encodeURIComponent(name)}`);
      })
      .catch((e: unknown) => {
        error = e instanceof Error ? e.message : String(e);
      });
  });
</script>

<section>
  {#if error}
    <p class="error">Não foi possível abrir o journal de hoje: {error}</p>
  {:else}
    <p>Redirecionando para o journal de hoje…</p>
  {/if}
</section>

<style>
  .error { color: #c33; }
</style>
