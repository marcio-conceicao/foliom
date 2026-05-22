// Tests for paste.ts — detectBulletTree (TS port of segment.rs Stage 1 bullet rule).

import { describe, it, expect } from 'vitest';
import { detectBulletTree } from '../paste';

describe('detectBulletTree', () => {
  it('returns null for plain text without bullets', () => {
    expect(detectBulletTree('hello world')).toBeNull();
  });

  it('returns null for a single bullet line (need >= 2)', () => {
    expect(detectBulletTree('- only one bullet')).toBeNull();
  });

  it('returns null for single bullet with trailing newline', () => {
    expect(detectBulletTree('- only one bullet\n')).toBeNull();
  });

  it('parses two root-level bullets', () => {
    const result = detectBulletTree('- a\n- b\n');
    expect(result).not.toBeNull();
    expect(result!.items.length).toBe(2);
    expect(result!.items[0].depth).toBe(0);
    expect(result!.items[0].raw).toBe('- a\n');
    expect(result!.items[1].depth).toBe(0);
    expect(result!.items[1].raw).toBe('- b\n');
  });

  it('parses TAB-indented bullets with correct depth', () => {
    const text = '\t- a\n\t- b\n';
    const result = detectBulletTree(text);
    expect(result).not.toBeNull();
    expect(result!.items.length).toBe(2);
    expect(result!.items[0].depth).toBe(1);
    expect(result!.items[0].raw).toBe('\t- a\n');
    expect(result!.items[1].depth).toBe(1);
    expect(result!.items[1].raw).toBe('\t- b\n');
  });

  it('parses mixed depth bullet tree', () => {
    const text = '- parent\n\t- child1\n\t- child2\n';
    const result = detectBulletTree(text);
    expect(result).not.toBeNull();
    expect(result!.items.length).toBe(3);
    expect(result!.items[0].depth).toBe(0);
    expect(result!.items[1].depth).toBe(1);
    expect(result!.items[2].depth).toBe(1);
  });

  it('continuation lines ride along inside raw of the bullet they follow', () => {
    // A continuation line (\t\t  content) belongs to the preceding bullet
    // Per segment.rs Stage 1: lines starting with TAB*2space are continuations
    const text = '- first\n  continuation\n- second\n';
    const result = detectBulletTree(text);
    expect(result).not.toBeNull();
    // Should produce 2 items; continuation is folded into first
    expect(result!.items.length).toBe(2);
    expect(result!.items[0].raw).toContain('continuation');
  });

  it('returns null when non-bullet, non-continuation line appears between bullets', () => {
    // A line that is neither a bullet nor a continuation should cause null
    const text = '- bullet1\nplain text here\n- bullet2\n';
    // Per D-30-07: mixed input falls back to default CM6 insert
    expect(detectBulletTree(text)).toBeNull();
  });

  it('handles text without trailing newline on last bullet', () => {
    const result = detectBulletTree('- a\n- b');
    // The parser should still work even without trailing newline
    expect(result).not.toBeNull();
    expect(result!.items.length).toBe(2);
  });

  it('round-trip: parse Foliom-style TAB hierarchy', () => {
    // Simulate a clipboard from copy-as-markdown (serializeBlockTree output)
    // The raw already has the TAB prefix + "- " + text + "\n"
    const text = '- root\n\t- child\n\t\t- grandchild\n';
    const result = detectBulletTree(text);
    expect(result).not.toBeNull();
    expect(result!.items.length).toBe(3);
    expect(result!.items.map((i) => i.depth)).toEqual([0, 1, 2]);
  });
});
