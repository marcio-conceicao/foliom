// Typed wrappers for the Phase 2 REST API exposed by the Foliom backend.
// All URLs are RELATIVE (`/api/...`) so the Vite dev proxy and rust-embed
// production server can both serve same-origin in their respective modes.

export interface PageSummary {
  name: string;
  isJournal: boolean;
  isResolved: boolean;
}

export interface DrawerRef {
  name: string;
  byteOffset: number;
  byteLength: number;
}

export interface Block {
  id: number;
  depth: number;
  raw: string;
  properties: Array<[string, string]>;
  drawers: DrawerRef[];
  children: Block[];
}

export interface PageDetail {
  name: string;
  isJournal: boolean;
  formattedTitle: string | null;
  blocks: Block[];
}

export interface Backlink {
  page: string;
  blockId: number;
  snippet: string;
}

export interface JournalEntry {
  date: string;
  name: string;
  formattedTitle: string;
}

export interface SearchHit {
  page: string;
  blockId: number;
  snippet: string;
}

export type SearchKind = 'content' | 'tag';

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url, { headers: { Accept: 'application/json' } });
  if (!res.ok) {
    throw new Error(`Foliom API ${res.status} ${res.statusText} for ${url}`);
  }
  return (await res.json()) as T;
}

export async function fetchPages(): Promise<PageSummary[]> {
  return getJson<PageSummary[]>('/api/pages');
}

export async function fetchPage(name: string): Promise<PageDetail> {
  return getJson<PageDetail>(`/api/pages/${encodeURIComponent(name)}`);
}

export async function fetchBacklinks(name: string): Promise<Backlink[]> {
  return getJson<Backlink[]>(`/api/pages/${encodeURIComponent(name)}/backlinks`);
}

export async function fetchPageTitles(): Promise<string[]> {
  return getJson<string[]>('/api/page-titles');
}

export async function fetchSearch(
  q: string,
  kind?: SearchKind,
  limit = 20,
): Promise<SearchHit[]> {
  const params = new URLSearchParams({ q, limit: String(limit) });
  if (kind) params.set('kind', kind);
  return getJson<SearchHit[]>(`/api/search?${params.toString()}`);
}

export async function fetchJournalsRange(
  from: string,
  to: string,
): Promise<JournalEntry[]> {
  const params = new URLSearchParams({ from, to });
  return getJson<JournalEntry[]>(`/api/journals?${params.toString()}`);
}

// Used by RedirectToday: the server returns a 302 to `/api/pages/{YYYY_MM_DD}`.
// Browsers will follow the redirect transparently for fetch; we surface the
// final URL so the caller can extract the journal page name.
export async function resolveJournalToday(): Promise<string> {
  const res = await fetch('/api/journals/today', { headers: { Accept: 'application/json' } });
  if (!res.ok) {
    throw new Error(`Foliom API ${res.status} resolving today's journal`);
  }
  // Final URL after redirect: `/api/pages/2026_05_21`
  const match = res.url.match(/\/api\/pages\/([^/?#]+)/);
  if (!match) {
    throw new Error(`Foliom API: unexpected redirect target ${res.url}`);
  }
  return decodeURIComponent(match[1]);
}
