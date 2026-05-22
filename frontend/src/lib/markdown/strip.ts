// frontend/src/lib/markdown/strip.ts
//
// Frontend mirror of `crates/core/src/parser/ast.rs::strip_segmenter_prefix`.
// Backend ships each block's `raw` field with the segmenter prefix INTACT
// (leading TABs, `- ` bullet on first line, `  ` continuation marker on the
// rest, plus any property lines and drawer ranges that fall inside the
// block's textual range). The renderer must strip those so markdown-it
// doesn't see `\t` as a CommonMark indented code block and so internal
// metadata (PRS-05 properties, PRS-06 drawers) stays invisible per the
// "no proprietary metadata" CLAUDE.md constraint.
//
// Behavior (must match Rust):
//   1. Leading TABs equal to `depth` are stripped on every line.
//   2. First line additionally drops the `- ` bullet marker.
//   3. Continuation lines additionally drop the `  ` (2-space) marker.
//   4. Property lines (`/^[A-Za-z][A-Za-z0-9._-]*::\s*/`) are dropped.
//   5. Drawer ranges (`:NAME:` ... `:END:` inclusive) are dropped.
//   6. `depth < 0` is the page prelude and renders nothing here — the
//      Block component only walks children for prelude rows.

const PROPERTY_LINE_RE = /^[A-Za-z][A-Za-z0-9._-]*::\s*/;
const DRAWER_OPEN_RE = /^:[A-Z]+:$/;

export interface DrawerRefLike {
  name: string;
  byteOffset: number;
  byteLength: number;
}

export function stripForRender(
  raw: string,
  depth: number,
  _properties: Array<[string, string]>,
  _drawers: DrawerRefLike[],
): string {
  if (depth < 0) return '';

  // Preserve trailing \n on each split chunk so re-join is exact.
  const lines = raw.split(/(?<=\n)/);
  const out: string[] = [];
  let inDrawer = false;

  lines.forEach((line, idx) => {
    // Step 1: skip ALL leading TABs (mirror of ast.rs::strip_segmenter_prefix
    // which is unbounded). The `depth` arg is kept on the signature for
    // future-proofing (prelude detection lives on it) but TAB count is
    // taken from the line itself.
    let i = 0;
    while (i < line.length && line[i] === '\t') i++;

    let body: string;
    if (idx === 0) {
      const rest = line.slice(i);
      if (rest.startsWith('- ')) {
        body = rest.slice(2);
      } else if (rest === '-' || rest === '-\n') {
        body = rest.endsWith('\n') ? '\n' : '';
      } else {
        body = line; // non-matching first line — pass through verbatim
      }
    } else {
      // Continuation: TABs already consumed; expect `  ` (2 spaces) as marker.
      if (line.length >= i + 2 && line[i] === ' ' && line[i + 1] === ' ') {
        body = line.slice(i + 2);
      } else {
        body = line.slice(i);
      }
    }

    const trimmed = body.replace(/\r?\n$/, '');

    // Drawer skip (range is INCLUSIVE on both ends).
    if (inDrawer) {
      if (trimmed === ':END:') inDrawer = false;
      return;
    }
    if (DRAWER_OPEN_RE.test(trimmed) && trimmed !== ':END:') {
      inDrawer = true;
      return;
    }

    // Property skip — these surface as the `properties` prop on the block.
    if (PROPERTY_LINE_RE.test(trimmed)) return;

    out.push(body);
  });

  return out.join('');
}
