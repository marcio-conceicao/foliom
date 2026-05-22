// frontend/src/lib/markdown/rules.ts
//
// markdown-it custom inline rules for Foliom:
//   - composite_tag: `#[[multi word tag]]` (must run before bare_tag so the
//     `#` is not consumed by it).
//   - page_link: `[[Page Name]]` (must run before markdown-it's default
//     `link` rule so the `[` is not mis-consumed).
//   - bare_tag: `#crypto` (with hex-color reject + URL-fragment guard).
//
// Heading suppression (PRS-04 invariant — see Pitfall 2 in 02-RESEARCH):
//   markdown-it runs inline rules inside ATX heading content by default.
//   We MUST NOT extract chips from headings.
//
//   **Spike decision: option 3** from 02-RESEARCH §Open Question 1
//   (post-processing token walker). Option 1 (env flag set/cleared inside
//   the block-rule wrapper) was tried first but did not work: markdown-it's
//   block ruler pushes an `inline` token with the raw heading content, then
//   runs the inline pass AFTER all block rules have completed — by then
//   `state.env.inHeading` has been cleared in the wrapper's `finally`, so
//   the inline rules never see the flag. Option 2 (separate md instance for
//   headings) requires two parser instances to stay in sync. Option 3
//   keeps a single instance and post-processes: after `md.parse`, walk the
//   token tree; for every `inline` token whose parent is a `heading_open`,
//   unwrap any foliom chip tokens (page_link / composite_tag / bare_tag)
//   back into plain text. Cleanest reversibility and localized to one
//   `core.ruler.push` hook.
//
// References:
//   - 02-RESEARCH.md §Markdown-it Custom Inline Rules (canonical pseudocode)
//   - crates/core/src/parser/ast.rs (Rust parser — same rejections)
//   - PRS-04 invariant: do not extract `#tag` from heading text.

import type MarkdownIt from 'markdown-it';
import type StateInline from 'markdown-it/lib/rules_inline/state_inline.mjs';
import Token from 'markdown-it/lib/token.mjs';

const TAG_FIRST_CHAR = /[A-Za-z0-9_À-￿]/;
const TAG_CONT_CHAR = /[A-Za-z0-9_\-/.À-￿]/;
// 3/6/8 hex digits are CSS colors — `#abcd` (4) and `#fff8` (4) are NOT colors
// and DO become tags per the spec.
const HEX_COLOR = /^[0-9a-fA-F]{3}$|^[0-9a-fA-F]{6}$|^[0-9a-fA-F]{8}$/;

function canonicalize(target: string): string {
  return target.replaceAll(/%2[Ff]/g, '/').normalize('NFC').trim();
}

function escapeHtml(s: string): string {
  return s
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

interface FoliomEnv {
  inHeading?: boolean;
}

function inHeading(state: StateInline): boolean {
  const env = state.env as FoliomEnv | undefined;
  return env?.inHeading === true;
}

// `#[[multi word tag]]` — MUST run before bare_tag.
export function compositeTag(state: StateInline, silent: boolean): boolean {
  if (inHeading(state)) return false;
  const src = state.src;
  const pos = state.pos;
  if (src.charCodeAt(pos) !== 0x23 /* # */) return false;
  if (src.charCodeAt(pos + 1) !== 0x5b /* [ */) return false;
  if (src.charCodeAt(pos + 2) !== 0x5b /* [ */) return false;

  // URL-fragment guard: `#` must not be preceded by an alphanumeric.
  if (pos > 0 && /[A-Za-z0-9]/.test(src[pos - 1])) return false;

  const close = src.indexOf(']]', pos + 3);
  if (close < 0) return false;

  const target = canonicalize(src.slice(pos + 3, close));
  if (!target) return false;

  if (!silent) {
    const tok = state.push('composite_tag', 'span', 0);
    tok.attrSet('class', 'tag composite');
    tok.attrSet('data-tag', target);
    tok.content = target;
  }
  state.pos = close + 2;
  return true;
}

// `[[Page Name]]`
export function pageLink(state: StateInline, silent: boolean): boolean {
  if (inHeading(state)) return false;
  const src = state.src;
  const pos = state.pos;
  if (src.charCodeAt(pos) !== 0x5b /* [ */) return false;
  if (src.charCodeAt(pos + 1) !== 0x5b /* [ */) return false;

  const close = src.indexOf(']]', pos + 2);
  if (close < 0) return false;

  const target = canonicalize(src.slice(pos + 2, close));
  if (!target) return false;

  if (!silent) {
    const tok = state.push('page_link', 'a', 0);
    tok.attrSet('class', 'page-link');
    tok.attrSet('data-page', target);
    tok.attrSet('href', `#/pages/${encodeURIComponent(target)}`);
    tok.content = target;
  }
  state.pos = close + 2;
  return true;
}

// `#bare-tag`
export function bareTag(state: StateInline, silent: boolean): boolean {
  if (inHeading(state)) return false;
  const src = state.src;
  const pos = state.pos;
  if (src.charCodeAt(pos) !== 0x23 /* # */) return false;

  // URL-fragment guard.
  if (pos > 0 && /[A-Za-z0-9]/.test(src[pos - 1])) return false;

  let i = pos + 1;
  if (i >= src.length || !TAG_FIRST_CHAR.test(src[i])) return false;
  i++;
  while (i < src.length && TAG_CONT_CHAR.test(src[i])) i++;

  // Strip trailing dots (sentence terminator).
  let end = i;
  while (end > pos + 1 && src[end - 1] === '.') end--;
  if (end === pos + 1) return false;

  const token = src.slice(pos + 1, end);
  if (HEX_COLOR.test(token)) return false;

  const target = canonicalize(token);
  if (!target) return false;

  if (!silent) {
    const tok = state.push('bare_tag', 'span', 0);
    tok.attrSet('class', 'tag');
    tok.attrSet('data-tag', target);
    tok.content = '#' + target;
  }
  state.pos = end;
  return true;
}

export function installFoliomRules(md: MarkdownIt): void {
  // Register the three inline rules. Order matters — `composite_tag` must
  // win against `bare_tag` for `#[[...]]`, and both must beat the default
  // `link` rule on `[[`.
  md.inline.ruler.before('link', 'composite_tag', compositeTag);
  md.inline.ruler.before('link', 'page_link', pageLink);
  md.inline.ruler.before('link', 'bare_tag', bareTag);

  // Render hooks. Every interpolated user-controlled value is escaped via
  // `escapeHtml` so the `{@html md.render(...)}` consumer cannot inject
  // markup through a crafted page name or tag name (T-02-12).
  md.renderer.rules.composite_tag = (tokens, idx) => {
    const target = escapeHtml(tokens[idx].attrGet('data-tag') || '');
    return `<span class="tag composite" data-tag="${target}">#${target}</span>`;
  };
  md.renderer.rules.page_link = (tokens, idx) => {
    const target = tokens[idx].attrGet('data-page') || '';
    const href = `#/pages/${encodeURIComponent(target)}`;
    const safe = escapeHtml(target);
    return `<a class="page-link" data-page="${safe}" href="${href}">${safe}</a>`;
  };
  md.renderer.rules.bare_tag = (tokens, idx) => {
    const target = escapeHtml(tokens[idx].attrGet('data-tag') || '');
    return `<span class="tag" data-tag="${target}">#${target}</span>`;
  };

  // Heading suppression (PRS-04) — option 3: post-process the token tree
  // after the inline pass has run. Any foliom chip token whose surrounding
  // inline token belongs to an `<h*>` block is rewritten back to a plain
  // `text` token so the heading reads literally.
  md.core.ruler.push('foliom_heading_strip', (state) => {
    const tokens = state.tokens;
    for (let i = 0; i < tokens.length; i++) {
      const t = tokens[i];
      if (t.type !== 'heading_open') continue;
      const inlineTok = tokens[i + 1];
      if (inlineTok?.type !== 'inline' || !inlineTok.children) continue;
      inlineTok.children = inlineTok.children.map((child) => {
        if (
          child.type === 'composite_tag' ||
          child.type === 'page_link' ||
          child.type === 'bare_tag'
        ) {
          const text = new Token('text', '', 0);
          // Reconstruct the literal source text the chip would have rendered.
          if (child.type === 'page_link') {
            text.content = child.attrGet('data-page') || '';
          } else {
            text.content = '#' + (child.attrGet('data-tag') || '');
          }
          return text;
        }
        return child;
      });
    }
  });
}
