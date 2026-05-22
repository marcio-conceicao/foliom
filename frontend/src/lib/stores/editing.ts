// Editing state store for single-block edit mode enforcement (EDT-01).
// At most one block can be in edit mode at a time.
//
// currentlyEditing: the id of the block currently in edit mode, or null.
// All Block.svelte instances watch this — when it changes away from their id,
// they unmount their CM6 editor (saving the doc first).

import { writable } from 'svelte/store';

/**
 * The block id currently in edit mode, or null if no block is being edited.
 * EDT-01: at most one block is in edit mode at any time.
 */
export const currentlyEditing = writable<number | null>(null);
