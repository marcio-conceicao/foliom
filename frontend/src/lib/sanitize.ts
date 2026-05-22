// frontend/src/lib/sanitize.ts
//
// Hand-rolled snippet sanitizer (T-02-20). The FTS5 backend hands us
// snippets that already contain <mark>…</mark> highlights, but the
// frontend defense-in-depth assumption is that the payload is hostile:
// we escape EVERYTHING first, then re-introduce only the two literal
// tokens `<mark>` and `</mark>` as safe HTML. Anything else — including
// `<MARK>`, `<mark class="x">`, `<script>`, `<img onerror=...>`, etc. —
// stays escaped as text.
//
// Why not DOMPurify? It would add ~20 KB gzipped to the bundle for a
// two-tag allow-list. The bundle budget matters (RNF-01 cold-start) and
// the surface here is deliberately narrow — backend owns highlight
// generation, frontend owns escape + restore. If the allow-list ever
// grows beyond a handful of tokens, swap to DOMPurify and delete this
// file.

const ESCAPE_MAP: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
};

function escapeHtml(raw: string): string {
  return raw.replace(/[&<>"']/g, (ch) => ESCAPE_MAP[ch]);
}

/**
 * Escape every special HTML character in `raw`, then re-introduce only
 * literal `<mark>` / `</mark>` tokens. Inputs are matched
 * case-sensitively against the canonical lowercase form the backend
 * emits (see crates/cli/src/cmd/serve/routes/search.rs).
 */
export function sanitizeSnippet(raw: string): string {
  if (!raw) return '';
  const escaped = escapeHtml(raw);
  return escaped
    .replace(/&lt;mark&gt;/g, '<mark>')
    .replace(/&lt;\/mark&gt;/g, '</mark>');
}
