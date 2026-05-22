/**
 * Vitest tests for the watcher SSE singleton + store transitions.
 *
 * SNC-06: conflict UI when external edit collides with foreground edit.
 *
 * Strategy:
 *   - We mock `globalThis.EventSource` with a controllable class BEFORE
 *     importing watcher.ts so the module captures the mock.
 *   - We use vi.mock to stub fetchPage from $lib/api.
 *   - We use vi.useFakeTimers() for the 10-second offline timer.
 *   - We use get() from svelte/store for assertions.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';

// ---------------------------------------------------------------------------
// Mock EventSource
// ---------------------------------------------------------------------------

type EventCallback = (e: Event) => void;

class MockEventSource {
  static instances: MockEventSource[] = [];

  url: string;
  readyState: number = 0; // CONNECTING
  private handlers: Map<string, EventCallback[]> = new Map();
  closeCount = 0;

  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSED = 2;

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  addEventListener(type: string, handler: EventCallback): void {
    if (!this.handlers.has(type)) this.handlers.set(type, []);
    this.handlers.get(type)!.push(handler);
  }

  removeEventListener(_type: string, _handler: EventCallback): void {
    // noop for tests
  }

  fire(type: string, eventInit?: EventInit & { data?: string }): void {
    const handlers = this.handlers.get(type) ?? [];
    let event: Event;
    if (eventInit?.data !== undefined) {
      event = new MessageEvent(type, eventInit as MessageEventInit);
    } else {
      event = new Event(type, eventInit);
    }
    for (const h of handlers) h(event);
  }

  close(): void {
    this.readyState = MockEventSource.CLOSED;
    this.closeCount++;
  }
}

// Assign mock BEFORE importing the module under test so the module captures
// the mock constructor.
// @ts-expect-error -- mock replaces global EventSource
globalThis.EventSource = MockEventSource;

// ---------------------------------------------------------------------------
// Mock fetchPage
// ---------------------------------------------------------------------------

vi.mock('../lib/api', () => ({
  fetchPage: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Imports (after mocks are set up)
// ---------------------------------------------------------------------------

import { watcherStatus, externalConflict } from '../lib/stores/watcher';
import { startWatcher, stopWatcher } from '../lib/watcher';
import { currentlyEditing } from '../lib/stores/editing';
import { currentPage } from '../lib/stores';
import { fetchPage } from '../lib/api';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function lastInstance(): MockEventSource {
  return MockEventSource.instances[MockEventSource.instances.length - 1];
}

function fireMessage(type: string, data: unknown): void {
  lastInstance().fire(type, { data: JSON.stringify(data) });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('watcher', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockEventSource.instances.length = 0;

    // Reset stores to initial state
    watcherStatus.set('reconnecting');
    externalConflict.set(null);
    currentlyEditing.set(null);
    currentPage.set(null);

    vi.mocked(fetchPage).mockResolvedValue({
      name: 'foo',
      isJournal: false,
      formattedTitle: null,
      blocks: [],
      fileHash: 'newhash',
      id: 1,
    });
  });

  afterEach(() => {
    stopWatcher();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Status transitions
  // -------------------------------------------------------------------------

  it('watcherStatus_transitions_on_open: fires open → connected', () => {
    startWatcher();
    const es = lastInstance();
    expect(get(watcherStatus)).toBe('reconnecting'); // initial before open

    es.fire('open');

    expect(get(watcherStatus)).toBe('connected');
  });

  it('watcherStatus_transitions_on_error: error → reconnecting, then offline after 10s', () => {
    startWatcher();
    const es = lastInstance();

    es.fire('error');
    expect(get(watcherStatus)).toBe('reconnecting');

    // Advance 9999ms — should still be reconnecting
    vi.advanceTimersByTime(9999);
    expect(get(watcherStatus)).toBe('reconnecting');

    // Advance past 10000ms — should now be offline
    vi.advanceTimersByTime(2);
    expect(get(watcherStatus)).toBe('offline');
  });

  it('open_after_error_clears_offline_timer: error then open prevents offline transition', () => {
    startWatcher();
    const es = lastInstance();

    es.fire('error');
    expect(get(watcherStatus)).toBe('reconnecting');

    // Open event should clear the offline timer
    es.fire('open');
    expect(get(watcherStatus)).toBe('connected');

    // Advance past 10s — should NOT transition to offline (timer was cleared)
    vi.advanceTimersByTime(11000);
    expect(get(watcherStatus)).toBe('connected');
  });

  // -------------------------------------------------------------------------
  // pages_updated: conflict path (block being edited)
  // -------------------------------------------------------------------------

  it('pages_updated_sets_externalConflict: editing block + page matches → set externalConflict', () => {
    currentPage.set({ name: 'foo', isJournal: false, formattedTitle: null, blocks: [], fileHash: 'oldhash', id: 1 });
    currentlyEditing.set(42);

    startWatcher();
    fireMessage('pages_updated', [{ name: 'foo', fileHash: 'abc' }]);

    expect(get(externalConflict)).toEqual({ newFileHash: 'abc' });
  });

  // -------------------------------------------------------------------------
  // pages_updated: silent reload path (no block being edited)
  // -------------------------------------------------------------------------

  it('pages_updated_no_conflict_when_not_editing: no edit → fetchPage called, no externalConflict', async () => {
    currentPage.set({ name: 'foo', isJournal: false, formattedTitle: null, blocks: [], fileHash: 'oldhash', id: 1 });
    currentlyEditing.set(null);

    startWatcher();
    fireMessage('pages_updated', [{ name: 'foo', fileHash: 'abc' }]);

    // Flush microtask queue so the fetchPage promise resolves
    await vi.runAllTimersAsync();

    expect(fetchPage).toHaveBeenCalledWith('foo');
    expect(get(externalConflict)).toBeNull();
  });

  // -------------------------------------------------------------------------
  // pages_updated: different page — should do nothing
  // -------------------------------------------------------------------------

  it('pages_updated_different_page_ignored: other page → no externalConflict, no fetchPage', () => {
    currentPage.set({ name: 'bar', isJournal: false, formattedTitle: null, blocks: [], fileHash: 'oldhash', id: 1 });
    currentlyEditing.set(null);

    startWatcher();
    fireMessage('pages_updated', [{ name: 'foo', fileHash: 'abc' }]);

    expect(get(externalConflict)).toBeNull();
    expect(fetchPage).not.toHaveBeenCalled();
  });

  // -------------------------------------------------------------------------
  // index_reset: always calls fetchPage for current page
  // -------------------------------------------------------------------------

  it('index_reset_calls_fetchPage: index_reset → fetchPage with currentPage.name', async () => {
    currentPage.set({ name: 'mypage', isJournal: false, formattedTitle: null, blocks: [], fileHash: 'h1', id: 2 });

    startWatcher();
    fireMessage('index_reset', {});

    await vi.runAllTimersAsync();

    expect(fetchPage).toHaveBeenCalledWith('mypage');
  });

  it('index_reset_noop_when_no_currentPage: index_reset with null currentPage → no fetchPage', async () => {
    currentPage.set(null);

    startWatcher();
    fireMessage('index_reset', {});

    await vi.runAllTimersAsync();

    expect(fetchPage).not.toHaveBeenCalled();
  });

  // -------------------------------------------------------------------------
  // Singleton: no duplicate EventSource
  // -------------------------------------------------------------------------

  it('no_duplicate_EventSource_on_double_startWatcher: second call is a no-op', () => {
    startWatcher();
    expect(MockEventSource.instances.length).toBe(1);

    const firstEs = lastInstance();
    firstEs.readyState = MockEventSource.OPEN; // simulate open state

    startWatcher(); // second call — should NOT create a new EventSource

    expect(MockEventSource.instances.length).toBe(1);
    expect(firstEs.closeCount).toBe(0); // first was not closed
  });

  // -------------------------------------------------------------------------
  // T-04-05: malformed JSON in pages_updated is discarded
  // -------------------------------------------------------------------------

  it('malformed_json_discarded: bad data → stores unchanged, no throw', () => {
    currentPage.set({ name: 'foo', isJournal: false, formattedTitle: null, blocks: [], fileHash: 'h', id: 1 });
    currentlyEditing.set(42);

    startWatcher();

    // Fire with invalid JSON — should not throw or corrupt stores
    lastInstance().fire('pages_updated', { data: '{not valid json' });

    expect(get(externalConflict)).toBeNull();
    expect(get(watcherStatus)).not.toBe('offline');
  });
});
