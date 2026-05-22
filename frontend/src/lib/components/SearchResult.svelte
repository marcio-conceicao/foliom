<script lang="ts">
  // Single search-result row. Snippet is rendered as innerHTML via a
  // hand-rolled allow-list sanitizer (`sanitizeSnippet`) — see the
  // T-02-20 mitigation note in SearchPalette.svelte. Only <mark> and
  // </mark> survive; every other tag is stripped.
  //
  // The row is presentational: parent owns the cursor index + click
  // handler so the keyboard/mouse paths converge on the same handler.

  import { sanitizeSnippet } from '../sanitize';

  let {
    page,
    snippet,
    active,
    onclick,
    onmouseenter,
  }: {
    page: string;
    snippet: string;
    active: boolean;
    onclick: () => void;
    onmouseenter: () => void;
  } = $props();
</script>

<!--
  svelte-ignore a11y_click_events_have_key_events
  Keyboard navigation lives on the parent <input> (ArrowUp/Down + Enter
  drive the cursor + activation). Per-row keydown would just duplicate
  that and steal focus from the input.
-->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<li
  class="result"
  class:active
  data-result
  data-page={page}
  {onclick}
  {onmouseenter}
  role="option"
  aria-selected={active}
>
  <strong class="page">{page}</strong>
  <span class="snippet">{@html sanitizeSnippet(snippet)}</span>
</li>
