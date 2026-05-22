import type { Component } from 'svelte';
import PageView from './lib/pages/PageView.svelte';
import JournalView from './lib/pages/JournalView.svelte';
import SearchView from './lib/pages/SearchView.svelte';
import NotFound from './lib/pages/NotFound.svelte';
import RedirectToday from './lib/pages/RedirectToday.svelte';

export const routes: Record<string, Component> = {
  '/': RedirectToday,
  '/pages/:name': PageView,
  '/journals/:date': JournalView,
  '/search': SearchView,
  '*': NotFound,
};
