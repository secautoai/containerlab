// Strato vendor identity: a hue per vendor, minimal stroke icons, and the
// strato wordmark. Ported from strato/src/vendors.jsx and keyed off the
// real template catalog (template.vendor / template.icon).

export const VENDOR_HUES: Record<string, number> = {
  Nokia: 190,
  'Open Source': 300,
  Cisco: 230,
  Arista: 35,
  Generic: 155,
  Fortinet: 25,
  Juniper: 120,
  VyOS: 260,
  'Palo Alto': 20,
  MikroTik: 205,
}

export const hueOf = (vendor: string | undefined): number => VENDOR_HUES[vendor ?? ''] ?? 210

export const hue = (h: number) => `oklch(0.72 0.15 ${h})`
export const hueBg = (h: number) => `oklch(0.72 0.15 ${h} / 0.16)`

// Icon paths (24x24 stroke outlines). router/switch/host/firewall are the
// Strato originals; the rest cover NetPilot's remaining icon names.
export const ICON_PATHS: Record<string, string> = {
  router:
    'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18M7.6 9.6 5.2 12l2.4 2.4M16.4 9.6 18.8 12l-2.4 2.4M9.6 7.6 12 5.2l2.4 2.4M9.6 16.4 12 18.8l2.4-2.4',
  switch: 'M3 7h18v10H3zM8 10.5h6M14.5 8.5l2 2-2 2M16 14.5h-6M9.5 12.5l-2 2 2 2',
  host: 'M4 5h16v10H4zM9 19h6M12 15v4',
  server: 'M4 5h16v10H4zM9 19h6M12 15v4',
  firewall:
    'M3 5h18v14H3zM3 9.7h18M3 14.3h18M9 5v4.7M15 5v4.7M6 9.7v4.6M12 9.7v4.6M18 9.7v4.6M9 14.3V19M15 14.3V19',
  cloud: 'M7 18a4.5 4.5 0 0 1-.4-9A6 6 0 0 1 18.2 10.6 3.8 3.8 0 0 1 17.4 18z',
  network: 'M12 3v5M5 21v-2a4 4 0 0 1 4-4h6a4 4 0 0 1 4 4v2M9 8h6v4H9zM12 12v3',
  internet:
    'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18M3 12h18M12 3c-2.6 2.6-2.6 15.4 0 18M12 3c2.6 2.6 2.6 15.4 0 18',
}

export function deviceIcon(iconName: string, size: number, color: string) {
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
      <path d={ICON_PATHS[iconName] ?? ICON_PATHS.server} />
    </svg>
  )
}

export function stratoMark({ size, inner, stroke = '#08211d' }: { size: number; inner?: string; stroke?: string }) {
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
  )
}
