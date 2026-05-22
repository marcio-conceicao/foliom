import { describe, it, expect } from 'vitest';
import { md } from '../index';

function render(src: string): string {
  return md.render(src);
}

describe('foliom inline rules — page links', () => {
  it('[[Foo]] → page-link chip', () => {
    const html = render('[[Foo]]');
    expect(html).toContain('class="page-link"');
    expect(html).toContain('data-page="Foo"');
    expect(html).toContain('href="#/pages/Foo"');
    expect(html).toContain('>Foo<');
  });

  it('[[Parent/Child]] → encoded href, decoded display + data-page', () => {
    const html = render('[[Parent/Child]]');
    expect(html).toContain('data-page="Parent/Child"');
    expect(html).toContain('href="#/pages/Parent%2FChild"');
    expect(html).toContain('>Parent/Child<');
  });

  it('[[Parent%2FChild]] → canonicalized identically to [[Parent/Child]]', () => {
    const html = render('[[Parent%2FChild]]');
    expect(html).toContain('data-page="Parent/Child"');
    expect(html).toContain('href="#/pages/Parent%2FChild"');
  });

  it('[[link with #hash]] keeps inner # literal — never re-parsed', () => {
    const html = render('[[link with #hash]]');
    expect(html).toContain('data-page="link with #hash"');
    // No bare-tag span emitted for the inner #hash
    expect(html).not.toContain('class="tag"');
  });
});

describe('foliom inline rules — bare tags', () => {
  it('#crypto → tag chip', () => {
    const html = render('#crypto');
    expect(html).toContain('class="tag"');
    expect(html).toContain('data-tag="crypto"');
    expect(html).toContain('>#crypto<');
  });

  it('#fim. → tag chip + literal `.`', () => {
    const html = render('say #fim.');
    expect(html).toContain('data-tag="fim"');
    // Trailing period left as text outside the chip
    expect(html).toMatch(/<\/span>\./);
  });

  it('#abcd (4 hex chars) IS a tag (only 3/6/8 are hex colors)', () => {
    const html = render('#abcd');
    expect(html).toContain('data-tag="abcd"');
  });
});

describe('foliom inline rules — composite tags', () => {
  it('#[[Monitoria de Qualidade]] → composite chip', () => {
    const html = render('#[[Monitoria de Qualidade]]');
    expect(html).toContain('class="tag composite"');
    expect(html).toContain('data-tag="Monitoria de Qualidade"');
    expect(html).toContain('>#Monitoria de Qualidade<');
  });
});

describe('foliom inline rules — rejections', () => {
  it('foo#bar (URL-fragment guard) → no tag chip', () => {
    const html = render('see foo#bar here');
    expect(html).not.toContain('class="tag"');
  });

  it('#fff (3-char hex color) → no tag chip', () => {
    const html = render('color #fff bright');
    expect(html).not.toContain('class="tag"');
  });

  it('#fff8 (4-char) → IS a tag (intentionally not a hex color)', () => {
    const html = render('#fff8');
    expect(html).toContain('data-tag="fff8"');
  });

  it('#ffffff (6-char hex) → no tag chip', () => {
    const html = render('#ffffff bg');
    expect(html).not.toContain('class="tag"');
  });

  it('`code with #tag` → no extraction inside code span', () => {
    const html = render('inline `code with #tag` here');
    expect(html).not.toContain('class="tag"');
  });

  it('### #Bruno (ATX heading) → no tag chip extracted from heading content', () => {
    const html = render('### #Bruno');
    expect(html).toContain('<h3>');
    expect(html).not.toContain('class="tag"');
  });
});

describe('foliom inline rules — XSS safety', () => {
  it('escapes < and > in page-link content', () => {
    const html = render('[[<script>]]');
    // The chip text should be escaped, no raw <script> element.
    expect(html).not.toMatch(/<script>/);
    expect(html).toContain('&lt;script&gt;');
  });
});
