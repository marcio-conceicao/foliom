/**
 * Singleton EventSource composable for Phase 4 disk-sync (plan 04-02).
 *
 * One EventSource per browser tab. Singleton guard prevents duplicate
 * connections on navigation (T-04-06, must_have: "Navigating between pages
 * does not create a second EventSource").
 *
 * Event handling:
 *   open        → watcherStatus = 'connected', clear offline timer
 *   error       → watcherStatus = 'reconnecting', start 10s offline timer
 *   pages_updated → if matched page being edited: set externalConflict
 *                   if matched page not being edited: fetchPage silently
 *   index_reset → fetchPage(currentPage.name) unconditionally
 *   page_deleted → no-op in this plan (T-04-07: out-of-scope for v1)
 *
 * T-04-05: JSON.parse wrapped in try/catch; malformed data is logged and
 * discarded without touching stores.
 */

import { get } from 'svelte/store';
import { fetchPage } from './api';
import { currentPage } from './stores';
import { currentlyEditing } from './stores/editing';
import { watcherStatus, externalConflict } from './stores/watcher';

// ---------------------------------------------------------------------------
// Module-level singleton
// ---------------------------------------------------------------------------

let es: EventSource | null = null;
let offlineTimer: ReturnType<typeof setTimeout> | null = null;

function clearOfflineTimer(): void {
  if (offlineTimer !== null) {
    clearTimeout(offlineTimer);
    offlineTimer = null;
  }
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

/** Shape of each entry in a pages_updated SSE data array. */
interface PageUpdatedEntry {
  name: string;
  fileHash: string;
}

function handleOpen(): void {
  watcherStatus.set('connected');
  clearOfflineTimer();
}

function handleError(): void {
  watcherStatus.set('reconnecting');
  clearOfflineTimer();
  offlineTimer = setTimeout(() => {
    watcherStatus.set('offline');
    offlineTimer = null;
  }, 10_000);
}

function handlePagesUpdated(e: Event): void {
  const raw = (e as MessageEvent).data;
  let pages: PageUpdatedEntry[];
  try {
    pages = JSON.parse(raw as string) as PageUpdatedEntry[];
  } catch {
    console.warn('[foliom/watcher] malformed pages_updated data — discarding', raw);
    return;
  }

  const page = get(currentPage);
  if (!page) return;

  const matched = pages.find((p) => p.name === page.name);
  if (!matched) return;

  if (get(currentlyEditing) !== null) {
    // Block is being edited → set conflict store so PageView can show the banner
    externalConflict.set({ newFileHash: matched.fileHash });
  } else {
    // No active editor → silent reload
    void fetchPage(page.name).then((fresh) => {
      currentPage.set(fresh);
    });
  }
}

function handleIndexReset(): void {
  const page = get(currentPage);
  if (!page) return;
  void fetchPage(page.name).then((fresh) => {
    currentPage.set(fresh);
  });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Start the singleton EventSource connecting to `/api/watch/events`.
 * If an active (non-CLOSED) EventSource already exists, returns immediately
 * without creating a second connection (T-04-06 singleton guard).
 *
 * Call once from App.svelte onMount.
 */
export function startWatcher(): void {
  if (es !== null && es.readyState !== (EventSource as typeof EventSource).CLOSED) {
    // Already connected or connecting — do not create a duplicate
    return;
  }

  es = new EventSource('/api/watch/events');

  es.addEventListener('open', handleOpen);
  es.addEventListener('error', handleError);
  es.addEventListener('pages_updated', handlePagesUpdated);
  es.addEventListener('index_reset', handleIndexReset);
}

/**
 * Stop the singleton EventSource and clear any pending offline timer.
 * Call from App.svelte beforeunload or in tests afterEach.
 */
export function stopWatcher(): void {
  clearOfflineTimer();
  if (es !== null) {
    es.close();
    es = null;
  }
}
