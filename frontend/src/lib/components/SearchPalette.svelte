<script lang="ts">
  // Stub — full implementation lands in Task 2 of plan 02-06. This
  // placeholder exists so App.svelte's `{#if paletteOpen}` block can
  // resolve the import after Task 1 (global keymap wiring) lands.
  // The Task 1 keys.test.ts suite never mounts this component.
  type Mode = 'modal' | 'inline';
  let { mode = 'modal' as Mode }: { mode?: Mode } = $props();
</script>

<div data-palette-stub data-mode={mode}></div>
