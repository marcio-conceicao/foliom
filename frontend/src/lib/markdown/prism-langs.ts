// frontend/src/lib/markdown/prism-langs.ts
//
// Statically import the 10 Prism languages locked by D-30. Doing this once
// at module load avoids the cost of Prism's auto-loader at render time and
// keeps the bundle deterministic.

import Prism from 'prismjs';

// `markup` covers HTML/XML/SVG; must be loaded before any language that
// inherits from it. Prism's own bootstrap already loads `markup`, but we
// import it explicitly so a tree-shaker that drops the bootstrap still
// sees the dependency.
import 'prismjs/components/prism-markup';
import 'prismjs/components/prism-css';
import 'prismjs/components/prism-clike';
import 'prismjs/components/prism-javascript';
import 'prismjs/components/prism-typescript';
import 'prismjs/components/prism-json';
import 'prismjs/components/prism-yaml';
import 'prismjs/components/prism-bash';
import 'prismjs/components/prism-rust';
import 'prismjs/components/prism-python';
import 'prismjs/components/prism-sql';

export { Prism };
