// VNC console tab: noVNC RFB client over the server's WS bridge.

import { useEffect, useRef, useState } from 'react'
// @ts-expect-error - noVNC ships without bundled types
import RFB from '@novnc/novnc'
import { wsUrl } from '../api'

export default function VncViewer({ labId, nodeId }: { labId: string; nodeId: string }) {
  const holder = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<'connecting' | 'connected' | 'failed'>('connecting')

  useEffect(() => {
    const el = holder.current
    if (!el) return
    let rfb: { disconnect(): void } | null = null
    try {
      const r = new RFB(el, wsUrl(`/api/ws/vnc/${labId}/${nodeId}`), { wsProtocols: [] })
      r.scaleViewport = true
      r.resizeSession = false
      r.background = '#0a0e14'
      r.addEventListener('connect', () => setStatus('connected'))
      r.addEventListener('disconnect', () => setStatus('failed'))
      rfb = r
    } catch {
      setStatus('failed')
    }
    return () => rfb?.disconnect()
  }, [labId, nodeId])

  return (
    <div className="relative h-full w-full bg-ink-950">
      <div ref={holder} className="h-full w-full" />
      {status !== 'connected' && (
        <div className="absolute inset-0 flex items-center justify-center text-xs text-ink-500">
          {status === 'connecting' ? 'Connecting to VNC…' : 'VNC unavailable — is the node running with a VNC console?'}
        </div>
      )}
    </div>
  )
}
