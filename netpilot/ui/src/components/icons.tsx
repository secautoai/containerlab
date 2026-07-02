// Device icon mapping (lucide) shared by palette + canvas.

import {
  Cloud,
  Globe,
  Network,
  Router,
  Server,
  Shield,
  Split,
  type LucideIcon,
} from 'lucide-react'

export const deviceIcon: Record<string, LucideIcon> = {
  router: Router,
  switch: Split,
  firewall: Shield,
  server: Server,
  cloud: Cloud,
  network: Network,
  internet: Globe,
}

export function iconFor(key: string): LucideIcon {
  return deviceIcon[key] ?? Server
}

export const stateColor: Record<string, string> = {
  stopped: '#64748b',
  starting: '#f59e0b',
  running: '#34d399',
  stopping: '#f59e0b',
  error: '#f87171',
}
