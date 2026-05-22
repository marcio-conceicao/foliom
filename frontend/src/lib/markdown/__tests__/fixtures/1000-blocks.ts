// Synthetic 1000-block fixture for the per-block render-perf assertion
// in plan 02-04. Layout: 10 top-level blocks, each with 100 children,
// every block carrying a small `raw` payload mirroring what the backend
// emits (segmenter prefix included so stripForRender exercises its TAB
// + bullet path).

import type { Block } from '../../../api';

export function buildThousandBlockTree(): Block[] {
  const tree: Block[] = [];
  let id = 1;
  for (let topIdx = 0; topIdx < 10; topIdx++) {
    const children: Block[] = [];
    for (let childIdx = 0; childIdx < 100; childIdx++) {
      children.push({
        id: id++,
        depth: 1,
        raw: `\t- Item ${topIdx}.${childIdx} with **bold** and [[LinkTarget${childIdx % 5}]]\n`,
        properties: [],
        drawers: [],
        children: [],
      });
    }
    tree.push({
      id: id++,
      depth: 0,
      raw: `- Top ${topIdx}\n`,
      properties: [],
      drawers: [],
      children,
    });
  }
  return tree;
}
