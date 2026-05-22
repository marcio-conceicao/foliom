import { describe, it, expect } from 'vitest';
import { md } from '../index';

describe('markdown-it — GFM + Prism', () => {
  it('renders **bold** and *italic*', () => {
    const html = md.render('**bold** and *italic*');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).toContain('<em>italic</em>');
  });

  it('renders GFM tables with thead and tbody', () => {
    const src = '| a | b |\n| - | - |\n| 1 | 2 |\n';
    const html = md.render(src);
    expect(html).toContain('<table>');
    expect(html).toContain('<thead>');
    expect(html).toContain('<tbody>');
  });

  it('renders fenced code with Prism, lang-label, and line-numbers class', () => {
    const src = '```rust\nfn main(){}\n```\n';
    const html = md.render(src);
    expect(html).toContain('language-rust');
    expect(html).toContain('line-numbers');
    expect(html).toContain('lang-label');
    // Prism token class for keyword
    expect(html).toMatch(/class="token (keyword|function)/);
  });

  it('code-fence highlighting does not introduce executable <script> for malicious code content', () => {
    const src = '```javascript\n<script>alert(1)</script>\n```\n';
    const html = md.render(src);
    // Raw <script> must not appear as an actual opening tag — it must be
    // escaped (Prism escapes `<` to `&lt;`; `>` is harmless in text context
    // when not preceded by `<`).
    expect(html).not.toMatch(/<script\b/);
    expect(html).toContain('&lt;');
  });

  it('linkifies bare URLs', () => {
    const html = md.render('see https://example.com here');
    expect(html).toContain('href="https://example.com"');
  });

  it('does NOT render raw HTML (html: false safety)', () => {
    const html = md.render('<b>raw</b>');
    expect(html).not.toContain('<b>raw</b>');
  });
});
