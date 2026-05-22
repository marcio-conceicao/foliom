// keys.test.ts — exercises the global keymap registrar wired in
// `lib/keys.ts`. The registrar attaches a single window-level keydown
// listener and toggles the `searchPalette` store on Ctrl/Cmd+K. Esc
// closes the palette only when open and is suppressed inside text
// inputs so it doesn't fight native input clearing.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { searchPalette } from '../../stores';
import { bindGlobalShortcuts } from '../../keys';

describe('lib/keys.ts — bindGlobalShortcuts', () => {
  let dispose: (() => void) | null = null;

  beforeEach(() => {
    searchPalette.set({ open: false, query: '' });
    dispose = bindGlobalShortcuts();
  });

  afterEach(() => {
    dispose?.();
    dispose = null;
    searchPalette.set({ open: false, query: '' });
    // Clean up any inputs created during tests.
    document.body.querySelectorAll('input,textarea').forEach((n) => n.remove());
  });

  it('Ctrl+K on window opens the palette', () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true }));
    expect(get(searchPalette).open).toBe(true);
  });

  it('Ctrl+K toggles closed when already open', () => {
    searchPalette.set({ open: true, query: 'foo' });
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true }));
    expect(get(searchPalette).open).toBe(false);
    expect(get(searchPalette).query).toBe('');
  });

  it('Meta+K (macOS) opens the palette', () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }));
    expect(get(searchPalette).open).toBe(true);
  });

  it('Ctrl+K fired from inside an <input> STILL opens the palette (modifier overrides input gate)', () => {
    const input = document.createElement('input');
    input.type = 'text';
    document.body.appendChild(input);
    input.focus();

    const ev = new KeyboardEvent('keydown', {
      key: 'k',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    // Dispatch on the input; the listener is on window so the event bubbles.
    input.dispatchEvent(ev);
    expect(get(searchPalette).open).toBe(true);
  });

  it('Esc inside an <input> does NOT close the palette (preserve native input clearing)', () => {
    searchPalette.set({ open: true, query: 'foo' });
    const input = document.createElement('input');
    input.type = 'text';
    document.body.appendChild(input);
    input.focus();

    const ev = new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    });
    input.dispatchEvent(ev);
    expect(get(searchPalette).open).toBe(true);
  });

  it('Esc on window closes an open palette', () => {
    searchPalette.set({ open: true, query: 'foo' });
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(get(searchPalette).open).toBe(false);
  });

  it('Esc on window when palette closed is a no-op (does NOT flip state)', () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(get(searchPalette).open).toBe(false);
  });

  it('disposer removes the listener', () => {
    dispose!();
    dispose = null;
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true }));
    expect(get(searchPalette).open).toBe(false);
  });
});
