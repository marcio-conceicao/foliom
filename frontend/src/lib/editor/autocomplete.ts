// CM6 completion source for [[page]] and #tag triggers.
// Implements D-30-06: [[  → pages only; # → tags + pages (labelled).
//
// The `from` field anchors replacement at `ctx.pos - prefix.length`,
// so only the typed prefix is replaced — the [[ or # delimiter is preserved.

import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';

/**
 * CM6 CompletionSource.
 * Wire into blockEditorExtensions via `autocompletion({ override: [completionSource] })`.
 */
export async function completionSource(
  ctx: CompletionContext,
): Promise<CompletionResult | null> {
  // Look back up to 64 chars to detect the trigger.
  const lookback = ctx.state.doc.sliceString(Math.max(0, ctx.pos - 64), ctx.pos);

  // [[ trigger — pages only (D-30-06).
  const bracketMatch = lookback.match(/\[\[([^\]]*)$/);
  if (bracketMatch) {
    const prefix = bracketMatch[1];
    try {
      const pages = await fetch(
        `/api/autocomplete?prefix=${encodeURIComponent(prefix)}&kind=page`,
      ).then((r) => r.json() as Promise<string[]>);
      return {
        from: ctx.pos - prefix.length,
        options: pages.map((p) => ({ label: p, type: 'page' })),
      };
    } catch {
      return null;
    }
  }

  // # trigger — tags + pages (D-30-06).
  const hashMatch = lookback.match(/(^|\s)#([\p{L}\p{N}_-]*)$/u);
  if (hashMatch) {
    const prefix = hashMatch[2];
    try {
      const items = await fetch(
        `/api/autocomplete?prefix=${encodeURIComponent(prefix)}&kind=all`,
      ).then((r) => r.json() as Promise<Array<{ name: string; kind: string }>>);
      return {
        from: ctx.pos - prefix.length,
        options: items.map((i) => ({
          label: i.name,
          type: i.kind,
          detail: i.kind === 'tag' ? 'tag' : 'page',
        })),
      };
    } catch {
      return null;
    }
  }

  return null;
}
