// Pure, generic undo/redo helpers used by the ClabStudio canvas history.
// Kept free of React/zustand so they can be unit-tested in isolation.

export const HISTORY_LIMIT = 50;

// recordPast appends the current present to the past stack (capped), for use
// right before applying a new edit. The caller clears the future separately.
export function recordPast<T>(past: T[], present: T, limit = HISTORY_LIMIT): T[] {
  const next = [...past, present];
  return next.length > limit ? next.slice(next.length - limit) : next;
}

export interface HistoryTransition<T> {
  past: T[];
  present: T;
  future: T[];
}

// applyUndo moves the last past state into present and pushes the current
// present onto the future. Returns null when there is nothing to undo.
export function applyUndo<T>(past: T[], present: T, future: T[]): HistoryTransition<T> | null {
  if (past.length === 0) return null;
  const newPast = past.slice(0, past.length - 1);
  const previous = past[past.length - 1];
  return { past: newPast, present: previous, future: [present, ...future] };
}

// applyRedo moves the first future state into present and pushes the current
// present onto the past. Returns null when there is nothing to redo.
export function applyRedo<T>(past: T[], present: T, future: T[]): HistoryTransition<T> | null {
  if (future.length === 0) return null;
  const [next, ...rest] = future;
  return { past: [...past, present], present: next, future: rest };
}

export const canUndo = <T>(past: T[]): boolean => past.length > 0;
export const canRedo = <T>(future: T[]): boolean => future.length > 0;
