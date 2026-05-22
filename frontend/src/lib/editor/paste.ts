// Paste detector: TS port of segment.rs Stage 1 bullet rule.
// Per D-30-07: if clipboard text has >= 2 bullet lines (^(\t*)- ), parse as
// a bullet tree; otherwise return null (fall back to default CM6 insert).
//
// This is intentionally MINIMAL: it only handles the Stage 1 bullet rule
// (leading TABs + "- " marker), not code fences or drawers. The clipboard
// content is assumed to come from serializeBlockTree or external text editor
// output that matches the same format.

export interface ParsedBulletItem {
  /** TAB-count depth (0 = root, 1 = one level nested, etc.) */
  depth: number;
  /** Full raw text of the block including leading TABs + "- " + content + "\n" */
  raw: string;
}

export interface ParsedBulletTree {
  items: ParsedBulletItem[];
}

/**
 * Detect whether clipboard text is a bullet hierarchy.
 *
 * Returns a `ParsedBulletTree` if the text has >= 2 bullet lines and every
 * non-continuation line is a bullet. Returns null otherwise.
 *
 * Bullet line:       `^\t*- ` (zero or more TABs followed by "- ")
 * Continuation line: `^\t+  ` (one or more TABs followed by two spaces)
 *                    OR `^  ` (two spaces, for depth-0 continuation)
 *
 * Non-bullet, non-continuation lines cause a null return per D-30-07.
 */
export function detectBulletTree(text: string): ParsedBulletTree | null {
  if (!text) return null;

  // Normalize: ensure text ends with a newline for uniform line processing.
  const normalized = text.endsWith('\n') ? text : text + '\n';
  const lines = normalized.split('\n');
  // Remove the empty string after the final newline.
  if (lines[lines.length - 1] === '') lines.pop();

  const items: ParsedBulletItem[] = [];
  let currentItem: ParsedBulletItem | null = null;

  for (const line of lines) {
    const lineWithNl = line + '\n';

    // Check if this is a bullet line: ^(\t*)-
    const bulletMatch = line.match(/^(\t*)- /);
    if (bulletMatch) {
      // Save previous item
      if (currentItem) items.push(currentItem);
      const depth = bulletMatch[1].length;
      currentItem = { depth, raw: lineWithNl };
      continue;
    }

    // Check if this is a continuation line belonging to the previous bullet.
    // Continuation: same TAB-indent as the parent bullet + 2 spaces,
    // OR empty line (empty lines also belong to the current block).
    if (currentItem !== null) {
      // Continuation: must be indented (TAB * depth + 2 spaces) or deeper
      // Per segment.rs: continuation lines begin with TAB*N + "  " (2 spaces)
      const contMatch = line.match(/^(\t+  |  )/);
      if (contMatch) {
        currentItem.raw += lineWithNl;
        continue;
      }
      // Empty lines are also continuations in segment.rs
      if (line === '') {
        currentItem.raw += lineWithNl;
        continue;
      }
    }

    // Non-bullet, non-continuation line: mixed content → null per D-30-07.
    return null;
  }

  // Save last item.
  if (currentItem) items.push(currentItem);

  // Need at least 2 bullet items per D-30-07.
  if (items.length < 2) return null;

  return { items };
}
