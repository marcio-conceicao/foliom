/**
 * Watcher stores for Phase 4 disk-sync (plan 04-02).
 *
 * watcherStatus: tracks SSE connection health (D-40-05).
 *   - 'reconnecting'  initial state; also set on EventSource 'error'
 *   - 'connected'     set on EventSource 'open'
 *   - 'offline'       set 10 seconds after 'error' with no recovery
 *
 * externalConflict: set by watcher.ts when a pages_updated event arrives for
 * the page currently being edited (SNC-06, D-40-04). PageView.svelte subscribes
 * to this store to surface the StaleConflict banner. Consumers must set it back
 * to null after handling to avoid a perpetual banner (T-04-06 mitigation).
 */

import { writable } from 'svelte/store';

/** SSE connection state shown by the Sidebar watcher-status pill. */
export const watcherStatus = writable<'connected' | 'reconnecting' | 'offline'>('reconnecting');

/**
 * Set when an external edit arrives for the currently-edited page.
 * Shape matches the pages_updated payload entry for the matched page.
 * Null means no pending conflict.
 */
export const externalConflict = writable<{ newFileHash: string } | null>(null);
