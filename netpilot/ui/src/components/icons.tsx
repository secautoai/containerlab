// Node runtime state → color, on the Strato palette (single source shared
// by the canvas cards and the Details panel).

export const stateColor: Record<string, string> = {
  stopped: 'var(--muted)',
  starting: 'var(--amber)',
  running: 'var(--green)',
  stopping: 'var(--amber)',
  error: 'var(--red)',
}
