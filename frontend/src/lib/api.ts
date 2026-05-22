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
  fileHash?: string; // hex BLAKE3 — present for resolved pages (plan 03-03)
  id?: number;       // page id — present for resolved pages (plan 03-03)
}

// --- Mutation API types (plan 03-03 wire contract) ---

/**
 * Response from PUT/POST/PATCH/DELETE /api/blocks.
 * The frontend replaces the local pageDetail.blocks subtree from blockSubtree
 * and updates pageDetail.fileHash — no follow-up GET needed.
 */
export interface MutationResponse {
  blockSubtree: Block[];
  fileHash: string;
  dirtyBlockIds: number[];
}

/** Response from POST /api/blocks (includes the new block's id). */
export interface CreateBlockResponse extends MutationResponse {
  id: number;
}

/**
 * Returned when the server responds with 409 Conflict.
 * The client should update its fileHash and surface a Reload banner.
 */
export interface StaleConflict {
  stale: true;
  currentFileHash: string;
}

/** Request body for PATCH /api/blocks/:id/structure */
export interface StructureReq {
  op: 'indent' | 'outdent' | 'move';
  prevHash: string;
  parentId?: number;
  ord?: number;
  depth?: number;
}

/** Request body for POST /api/blocks */
export interface NewBlockReq {
  pageId: number;
  parentId: number | null;
  ord: number;
  depth: number;
  raw: string;
  prevHash: string;
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

// --- Mutation wrappers (plan 03-04) ---
// Each wrapper handles the 409 Stale response by returning a StaleConflict
// object instead of throwing, so callers can surface the Reload banner.
// All other non-ok responses throw an Error.

/**
 * PUT /api/blocks/:id — update block raw text.
 * Returns MutationResponse on success, StaleConflict on 409.
 */
export async function putBlock(
  id: number,
  raw: string,
  prevHash: string,
): Promise<MutationResponse | StaleConflict> {
  const res = await fetch(`/api/blocks/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify({ raw, prevHash }),
  });
  if (res.status === 409) {
    const body = (await res.json()) as { error: string; currentFileHash: string };
    return { stale: true, currentFileHash: body.currentFileHash };
  }
  if (!res.ok) {
    throw new Error(`Foliom API PUT /api/blocks/${id} → ${res.status} ${res.statusText}`);
  }
  return (await res.json()) as MutationResponse;
}

/**
 * POST /api/blocks — create a new block.
 * Returns CreateBlockResponse on success, StaleConflict on 409.
 */
export async function postBlock(
  req: NewBlockReq,
): Promise<CreateBlockResponse | StaleConflict> {
  const res = await fetch('/api/blocks', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(req),
  });
  if (res.status === 409) {
    const body = (await res.json()) as { error: string; currentFileHash: string };
    return { stale: true, currentFileHash: body.currentFileHash };
  }
  if (!res.ok) {
    throw new Error(`Foliom API POST /api/blocks → ${res.status} ${res.statusText}`);
  }
  return (await res.json()) as CreateBlockResponse;
}

/**
 * PATCH /api/blocks/:id/structure — indent, outdent, or move a block.
 * Returns MutationResponse on success, StaleConflict on 409.
 */
export async function patchBlockStructure(
  id: number,
  req: StructureReq,
): Promise<MutationResponse | StaleConflict> {
  const res = await fetch(`/api/blocks/${id}/structure`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(req),
  });
  if (res.status === 409) {
    const body = (await res.json()) as { error: string; currentFileHash: string };
    return { stale: true, currentFileHash: body.currentFileHash };
  }
  if (!res.ok) {
    throw new Error(`Foliom API PATCH /api/blocks/${id}/structure → ${res.status} ${res.statusText}`);
  }
  return (await res.json()) as MutationResponse;
}

/**
 * DELETE /api/blocks/:id — delete a block (EDT-06 empty-block, D-30-08).
 * Returns MutationResponse on success, StaleConflict on 409.
 */
export async function deleteBlock(
  id: number,
  prevHash: string,
): Promise<MutationResponse | StaleConflict> {
  const res = await fetch(`/api/blocks/${id}?prevHash=${encodeURIComponent(prevHash)}`, {
    method: 'DELETE',
    headers: { Accept: 'application/json' },
  });
  if (res.status === 409) {
    const body = (await res.json()) as { error: string; currentFileHash: string };
    return { stale: true, currentFileHash: body.currentFileHash };
  }
  if (!res.ok) {
    throw new Error(`Foliom API DELETE /api/blocks/${id} → ${res.status} ${res.statusText}`);
  }
  // DELETE may return 204 No Content or 200 MutationResponse
  if (res.status === 204) {
    return { blockSubtree: [], fileHash: '', dirtyBlockIds: [] };
  }
  return (await res.json()) as MutationResponse;
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

// ─── Plan 03-06: Page create + rename ────────────────────────────────────────

export interface RenamePageResponse {
  rewrittenCount: number;
  warnings: string[];
}

/**
 * POST /api/pages — create a new empty page.
 * If `name` matches YYYY_MM_DD, the page lands in journals/.
 * The file is created with `- \n` (one empty bullet).
 */
export async function createPage(name: string): Promise<PageSummary> {
  const res = await fetch('/api/pages', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify({ name }),
  });
  if (res.status === 400) {
    const body = (await res.json()) as { error: string };
    const err = Object.assign(new Error(body.error), { status: 400 });
    throw err;
  }
  if (!res.ok) {
    throw Object.assign(
      new Error(`Foliom API POST /api/pages → ${res.status} ${res.statusText}`),
      { status: res.status },
    );
  }
  return (await res.json()) as PageSummary;
}

/**
 * POST /api/pages/:name/rename — rename a page with optional backlink rewrite.
 * Throws with `status: 409` if the target name already exists as a backed page.
 * Throws with `status: 400` if the new name is invalid.
 */
export async function renamePage(
  oldName: string,
  newName: string,
  rewriteBacklinks: boolean,
): Promise<RenamePageResponse> {
  const res = await fetch(`/api/pages/${encodeURIComponent(oldName)}/rename`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify({ newName, rewriteBacklinks }),
  });
  if (res.status === 409 || res.status === 400) {
    const body = (await res.json()) as { error: string };
    throw Object.assign(new Error(body.error), { status: res.status });
  }
  if (!res.ok) {
    throw Object.assign(
      new Error(`Foliom API POST /api/pages/:name/rename → ${res.status} ${res.statusText}`),
      { status: res.status },
    );
  }
  return (await res.json()) as RenamePageResponse;
}
