// Copy-as-markdown serializer.
// Per 03-RESEARCH §6 and EDT-11: block.raw is verbatim from segment.rs Stage 1
// so it already includes leading TABs + "- " + content + "\n".
// serializeBlockTree is the exact inverse of detectBulletTree in paste.ts.

import type { Block } from '../api';

/**
 * Serialize a block and its children to a clipboard-ready markdown string.
 *
 * The output is verbatim concatenation of `block.raw` (depth-first).
 * Pasting this into Foliom will round-trip through detectBulletTree correctly
 * because each item has the correct TAB-depth prefix.
 *
 * @example
 * // Given block: { raw: "- parent\n", children: [{ raw: "\t- child\n", children: [] }] }
 * // Returns: "- parent\n\t- child\n"
 */
export function serializeBlockTree(block: Block): string {
  // block.raw already includes leading \t*- and trailing \n (verbatim from segment.rs)
  const self = block.raw;
  const kids = block.children.map(serializeBlockTree).join('');
  return self + kids;
}
