// ThemeToggle tests — mounts the toggle, simulates clicks on each of the
// three states, and asserts the `theme` store + `<html data-theme>` + the
// localStorage `theme` key all stay in sync.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mount, unmount, tick } from 'svelte';
import { get } from 'svelte/store';
import ThemeToggle from '../ThemeToggle.svelte';
import { theme } from '../../stores';

function newTarget(): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  return host;
}

describe('ThemeToggle.svelte', () => {
  beforeEach(() => {
    localStorage.clear();
    theme.set('auto');
    document.documentElement.removeAttribute('data-theme');
  });

  afterEach(() => {
    document.documentElement.removeAttribute('data-theme');
  });

  it('renders three buttons: Claro / Auto / Escuro', () => {
    const target = newTarget();
    const app = mount(ThemeToggle, { target, props: {} });
    const buttons = target.querySelectorAll('button');
    expect(buttons.length).toBe(3);
    const labels = Array.from(buttons).map((b) => b.textContent?.trim());
    expect(labels).toContain('Claro');
    expect(labels).toContain('Auto');
    expect(labels).toContain('Escuro');
    unmount(app);
  });

  it('clicking Escuro sets theme store, <html data-theme>, and localStorage', async () => {
    const target = newTarget();
    const app = mount(ThemeToggle, { target, props: {} });

    const dark = Array.from(target.querySelectorAll('button')).find(
      (b) => b.textContent?.trim() === 'Escuro',
    ) as HTMLButtonElement;
    expect(dark).toBeDefined();
    dark.click();
    await tick();

    expect(get(theme)).toBe('dark');
    expect(localStorage.getItem('theme')).toBe('dark');
    // The App-level $effect applies data-theme; ThemeToggle itself updates the
    // attribute via a local applyResolvedTheme helper so the toggle works in
    // isolation in tests.
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');

    unmount(app);
  });

  it('clicking Claro sets data-theme=light', async () => {
    const target = newTarget();
    const app = mount(ThemeToggle, { target, props: {} });

    const light = Array.from(target.querySelectorAll('button')).find(
      (b) => b.textContent?.trim() === 'Claro',
    ) as HTMLButtonElement;
    light.click();
    await tick();

    expect(get(theme)).toBe('light');
    expect(localStorage.getItem('theme')).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');

    unmount(app);
  });

  it('marks the active button with aria-pressed=true', async () => {
    const target = newTarget();
    const app = mount(ThemeToggle, { target, props: {} });

    const dark = Array.from(target.querySelectorAll('button')).find(
      (b) => b.textContent?.trim() === 'Escuro',
    ) as HTMLButtonElement;
    dark.click();
    await tick();

    expect(dark.getAttribute('aria-pressed')).toBe('true');
    const auto = Array.from(target.querySelectorAll('button')).find(
      (b) => b.textContent?.trim() === 'Auto',
    ) as HTMLButtonElement;
    expect(auto.getAttribute('aria-pressed')).toBe('false');

    unmount(app);
  });
});
