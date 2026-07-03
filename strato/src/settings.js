// Design-container props (Appearance / Agent / Canvas sections), overridable
// via URL params for tinkering: ?accent=%235a7ff0&speed=2&packets=0
const q = new URLSearchParams(window.location.search);

export const SETTINGS = {
  accent: q.get('accent') || '#38d1ba',
  agentSpeed: Math.min(4, Math.max(0.5, parseFloat(q.get('speed') || '1') || 1)),
  packets: q.get('packets') !== '0',
};
