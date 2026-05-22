// frontend/src/lib/markdown/index.ts
//
// Configured markdown-it instance for per-block rendering. Used by
// `Block.svelte` via `{@html md.render(stripForRender(raw, ...))}`.
//
// Safety posture:
//   - `html: false` — never trust raw HTML embedded in .md files (T-02-12).
//   - All foliom-rule render hooks escape user-controlled values.
//   - Prism highlight escapes code content via its own tokenizer (T-02-13).
//
// Highlight output structure:
//   <pre class="language-{lang} line-numbers">
//     <code class="language-{lang}">{tokenized html}</code>
//     <span class="lang-label">{lang}</span>
//   </pre>
//
// The `line-numbers` class is picked up by `prism-foliom.css` which numbers
// each `<code>` line via CSS counters (no JS plugin needed since we render
// once and never re-process).

import MarkdownIt from 'markdown-it';
import { installFoliomRules } from './rules';
import { Prism } from './prism-langs';

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function highlight(str: string, lang: string): string {
  if (lang && Prism.languages[lang]) {
    const html = Prism.highlight(str, Prism.languages[lang], lang);
    const safeLang = escapeHtml(lang);
    return (
      `<pre class="language-${safeLang} line-numbers">` +
      `<code class="language-${safeLang}">${html}</code>` +
      `<span class="lang-label">${safeLang}</span>` +
      `</pre>`
    );
  }
  // Unknown lang / no fence info — escape and emit a plain block.
  return `<pre><code>${escapeHtml(str)}</code></pre>`;
}

export const md: MarkdownIt = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
  breaks: false,
  highlight,
});

installFoliomRules(md);
