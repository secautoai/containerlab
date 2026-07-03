import React from 'react';

export const VENDORS = {
  srl: { label: 'Nokia SR Linux', abbr: 'SRL', hue: 190, file: (n) => n.toLowerCase() + '.srl.cfg' },
  frr: { label: 'FRR 10.1', abbr: 'FRR', hue: 300, file: () => 'frr.conf' },
  ios: { label: 'Cisco IOL', abbr: 'IOS', hue: 230, file: (n) => n.toLowerCase() + '.ios.cfg' },
  eos: { label: 'Arista cEOS', abbr: 'EOS', hue: 35, file: (n) => n.toLowerCase() + '.eos.cfg' },
  linux: { label: 'Alpine Linux', abbr: 'LNX', hue: 155, file: () => 'interfaces' },
  fw: { label: 'FortiOS 7.6', abbr: 'FGT', hue: 25, file: (n) => n.toLowerCase() + '.fgt.cfg' },
};

export const ICONS = {
  router:
    'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18M7.6 9.6 5.2 12l2.4 2.4M16.4 9.6 18.8 12l-2.4 2.4M9.6 7.6 12 5.2l2.4 2.4M9.6 16.4 12 18.8l2.4-2.4',
  switch: 'M3 7h18v10H3zM8 10.5h6M14.5 8.5l2 2-2 2M16 14.5h-6M9.5 12.5l-2 2 2 2',
  host: 'M4 5h16v10H4zM9 19h6M12 15v4',
  firewall:
    'M3 5h18v14H3zM3 9.7h18M3 14.3h18M9 5v4.7M15 5v4.7M6 9.7v4.6M12 9.7v4.6M18 9.7v4.6M9 14.3V19M15 14.3V19',
};

export const hue = (h) => `oklch(0.72 0.15 ${h})`;
export const hueBg = (h) => `oklch(0.72 0.15 ${h} / 0.16)`;

export function deviceIcon(type, size, color) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ display: 'block' }}
    >
      <path d={ICONS[type]} />
    </svg>
  );
}

export function stratoMark({ size, inner, stroke = '#08211d' }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={inner || stroke}
      strokeWidth={2.4}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M3 14c4-8 14-8 18 0M7 14c2.5-4.5 7.5-4.5 10 0M12 14v.01" />
    </svg>
  );
}
