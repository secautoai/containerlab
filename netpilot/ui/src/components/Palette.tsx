// Device palette: templates grouped by vendor, dragged onto the canvas.

import { useMemo, useState } from 'react'
import { ChevronDown, ChevronRight, Search, X } from 'lucide-react'
import { useStore } from '../store'
import { deviceIcon, hue, hueBg, hueOf } from '../vendors'

export default function Palette({ onClose }: { onClose?: () => void }) {
  const templates = useStore((s) => s.templates)
  const [filter, setFilter] = useState('')
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})

  const groups = useMemo(() => {
    const q = filter.toLowerCase()
    const filtered = templates.filter(
      (t) =>
        !q ||
        t.name.toLowerCase().includes(q) ||
        t.vendor.toLowerCase().includes(q) ||
        t.id.includes(q),
    )
    const byVendor = new Map<string, typeof filtered>()
    for (const t of filtered) {
      const key = t.vendor || 'Other'
      if (!byVendor.has(key)) byVendor.set(key, [])
      byVendor.get(key)!.push(t)
    }
    return [...byVendor.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [templates, filter])

  return (
    <aside className="flex h-full w-56 shrink-0 flex-col border-r border-ink-800 bg-ink-900">
      <div className="border-b border-ink-800 p-2">
        <div className="flex items-center gap-2 rounded-lg bg-ink-950 px-2 py-1.5">
          <Search size={14} className="text-ink-600" />
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Search devices…"
            className="w-full bg-transparent text-xs text-ink-200 outline-none placeholder:text-ink-600"
          />
          {onClose && (
            <button onClick={onClose} className="rounded p-0.5 text-ink-400 hover:text-white" title="Close">
              <X size={13} />
            </button>
          )}
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-2">
        {groups.map(([vendor, list]) => (
          <div key={vendor} className="mb-1">
            <button
              className="flex w-full items-center gap-1 px-1 py-1 text-[10px] font-semibold uppercase tracking-wider text-ink-400 hover:text-ink-200"
              onClick={() => setCollapsed((c) => ({ ...c, [vendor]: !c[vendor] }))}
            >
              {collapsed[vendor] ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
              {vendor}
            </button>
            {!collapsed[vendor] &&
              list.map((t) => {
                const h = hueOf(t.vendor)
                const hasImage = t.available_images.length > 0
                return (
                  <div
                    key={t.id}
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData('application/netpilot-template', t.id)
                      e.dataTransfer.effectAllowed = 'copy'
                    }}
                    title={hasImage ? t.notes : `${t.notes}\n\n⚠ no image uploaded (images/${t.id}/<version>/)`}
                    className="group flex cursor-grab items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-ink-800 active:cursor-grabbing"
                  >
                    <div
                      className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md"
                      style={{ background: hueBg(h) }}
                    >
                      {deviceIcon(t.icon, 15, hue(h))}
                    </div>
                    <div className="min-w-0">
                      <div className="truncate text-xs text-ink-200">{t.name}</div>
                      <div className="text-[9px] text-ink-600">
                        {hasImage ? (
                          <span className="text-emerald-500">{t.available_images.length} image(s)</span>
                        ) : (
                          <span className="text-amber-600">no image</span>
                        )}
                        {' · '}
                        {t.ram_mb >= 1024 ? `${t.ram_mb / 1024}G` : `${t.ram_mb}M`}
                      </div>
                    </div>
                  </div>
                )
              })}
          </div>
        ))}
        {groups.length === 0 && <p className="p-2 text-xs text-ink-600">No matches.</p>}
      </div>
      <p className="border-t border-ink-800 p-2 text-[10px] leading-relaxed text-ink-600">
        Drag a device onto the canvas. Drag between devices to cable them.
      </p>
    </aside>
  )
}
