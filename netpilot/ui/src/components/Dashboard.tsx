// Lab dashboard: system status bar, lab cards, create/import.

import { useEffect, useRef, useState } from 'react'
import {
  Activity,
  Boxes,
  Copy,
  Cpu,
  FileUp,
  HardDrive,
  Plus,
  Trash2,
} from 'lucide-react'
import { api, type LabSummary, type SystemStatus } from '../api'
import { useStore } from '../store'
import { stratoMark } from '../vendors'
import ImageManager from './ImageManager'

export default function Dashboard() {
  const [labs, setLabs] = useState<LabSummary[]>([])
  const [system, setSystem] = useState<SystemStatus | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [showImages, setShowImages] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const openLab = useStore((s) => s.openLab)
  const fileRef = useRef<HTMLInputElement>(null)

  const refresh = async () => {
    try {
      const [l, s] = await Promise.all([api.labs(), api.system()])
      setLabs(l)
      setSystem(s)
    } catch (e) {
      setError(String(e))
    }
  }

  useEffect(() => {
    void refresh()
  }, [])

  const importFile = async (file: File) => {
    try {
      const lab = await api.import(file)
      await openLab(lab.id)
    } catch (e) {
      setError(`Import failed: ${e instanceof Error ? e.message : e}`)
    }
  }

  return (
    <div className="h-full overflow-y-auto">
      <header className="sticky top-0 z-10 border-b border-ink-800 bg-ink-950/90 backdrop-blur">
        <div className="mx-auto flex max-w-6xl items-center gap-4 px-6 py-4">
          <div className="flex items-center gap-2.5">
            <div
              className="flex h-9 w-9 items-center justify-center rounded-lg"
              style={{ background: 'linear-gradient(135deg,var(--accent),#2a8fd1)' }}
            >
              {stratoMark({ size: 19 })}
            </div>
            <div>
              <h1
                className="text-lg font-semibold text-white"
                style={{ fontFamily: "'Space Grotesk','IBM Plex Sans',sans-serif", letterSpacing: '-.3px' }}
              >
                strato
                <span
                  className="ml-2 align-middle text-[10px] font-semibold tracking-wide"
                  style={{ color: 'var(--accent)', background: 'var(--accentSoft)', borderRadius: 5, padding: '2px 7px' }}
                >
                  AI NETWORK ENGINEER
                </span>
              </h1>
              <p className="text-xs text-ink-400">describe a network — it builds, validates, and runs itself · powered by NetPilot</p>
            </div>
          </div>
          <div className="ml-auto flex items-center gap-3 text-xs text-ink-400">
            {system && (
              <>
                <span className="flex items-center gap-1.5" title="hardware acceleration">
                  <Cpu size={14} className={system.kvm ? 'text-emerald-400' : 'text-amber-400'} />
                  {system.kvm ? 'KVM' : 'TCG'}
                </span>
                <span
                  className="flex items-center gap-1.5"
                  title="VM nodes (QEMU) · container nodes (docker) · native FRR"
                >
                  <Activity size={14} />
                  <span className={system.qemu_available ? 'text-emerald-400' : 'text-ink-600'}>qemu</span>
                  <span className={system.docker_available ? 'text-emerald-400' : 'text-ink-600'}>docker</span>
                  <span className={system.frr_available ? 'text-emerald-400' : 'text-ink-600'}>frr</span>
                </span>
                <span className="rounded bg-ink-800 px-1.5 py-0.5 text-[10px]" title="datapath mode">
                  {system.datapath}
                </span>
                {system.ai.available && (
                  <span className="rounded bg-accent-600/20 px-1.5 py-0.5 text-[10px] text-accent-500" title="AI agent model">
                    {system.ai.model.split('/').pop()}
                  </span>
                )}
                <span className="flex items-center gap-1.5">
                  <Boxes size={14} /> {system.running_nodes} running
                </span>
                <span className="flex items-center gap-1.5">
                  <HardDrive size={14} /> {system.images} images
                </span>
              </>
            )}
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-6xl px-6 py-8">
        {error && (
          <div className="mb-4 rounded-lg border border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
            {error}
            <button className="ml-3 text-red-400 underline" onClick={() => setError(null)}>
              dismiss
            </button>
          </div>
        )}

        <div className="mb-6 flex items-center gap-3">
          <h2 className="text-sm font-medium uppercase tracking-wider text-ink-400">Labs</h2>
          <div className="ml-auto flex gap-2">
            <button
              onClick={() => setShowImages(true)}
              className="flex items-center gap-2 rounded-lg border border-ink-700 px-3 py-1.5 text-sm text-ink-300 hover:border-ink-600 hover:text-white"
            >
              <HardDrive size={15} /> Images
            </button>
            <button
              onClick={() => fileRef.current?.click()}
              className="flex items-center gap-2 rounded-lg border border-ink-700 px-3 py-1.5 text-sm text-ink-300 hover:border-ink-600 hover:text-white"
            >
              <FileUp size={15} /> Import
            </button>
            <input
              ref={fileRef}
              type="file"
              accept=".zip,.yaml,.yml,.unl"
              className="hidden"
              onChange={(e) => {
                const f = e.target.files?.[0]
                if (f) void importFile(f)
                e.target.value = ''
              }}
            />
            <button
              onClick={() => setShowCreate(true)}
              className="flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm font-semibold"
              style={{ background: 'var(--accent)', color: '#08211d' }}
            >
              <Plus size={15} /> New Lab
            </button>
          </div>
        </div>

        {labs.length === 0 ? (
          <div className="rounded-xl border border-dashed border-ink-700 py-20 text-center">
            <p className="text-ink-400">No labs yet.</p>
            <p className="mt-1 text-sm text-ink-600">
              Create one, or import an EVE-NG .unl / containerlab YAML / NetPilot zip.
            </p>
          </div>
        ) : (
          Object.entries(
            labs.reduce<Record<string, LabSummary[]>>((acc, lab) => {
              const key = lab.folder && lab.folder !== '/' ? lab.folder : ''
              ;(acc[key] ??= []).push(lab)
              return acc
            }, {}),
          )
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([folder, group]) => (
              <section key={folder || '(root)'} className="mb-8">
                {folder && (
                  <h3 className="mb-3 text-xs font-semibold uppercase tracking-wider text-ink-500">
                    📁 {folder}
                  </h3>
                )}
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  {group.map((lab) => (
              <div
                key={lab.id}
                onClick={() => void openLab(lab.id)}
                className="group cursor-pointer rounded-xl border border-ink-800 bg-ink-900 p-4 transition hover:border-accent-600/60 hover:bg-ink-850"
              >
                <div className="flex items-start justify-between">
                  <h3 className="font-medium text-white group-hover:text-accent-500">{lab.name}</h3>
                  <div className="flex gap-1 opacity-0 transition group-hover:opacity-100">
                    <button
                      title="Clone"
                      className="rounded p-1 text-ink-400 hover:bg-ink-700 hover:text-white"
                      onClick={async (e) => {
                        e.stopPropagation()
                        await api.cloneLab(lab.id)
                        void refresh()
                      }}
                    >
                      <Copy size={14} />
                    </button>
                    <button
                      title="Delete"
                      className="rounded p-1 text-ink-400 hover:bg-red-900/50 hover:text-red-300"
                      onClick={async (e) => {
                        e.stopPropagation()
                        if (confirm(`Delete lab "${lab.name}"? This removes its disks too.`)) {
                          await api.deleteLab(lab.id)
                          void refresh()
                        }
                      }}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
                <p className="mt-1 line-clamp-2 min-h-8 text-sm text-ink-400">
                  {lab.description || 'No description'}
                </p>
                <div className="mt-3 flex items-center gap-3 text-xs text-ink-600">
                  <span>{lab.node_count} nodes</span>
                  <span>·</span>
                  <span>updated {new Date(lab.modified_at).toLocaleString()}</span>
                </div>
              </div>
                  ))}
                </div>
              </section>
            ))
        )}
      </main>

      {showCreate && <CreateLabModal onClose={() => setShowCreate(false)} onCreated={(id) => void openLab(id)} />}
      {showImages && <ImageManager onClose={() => setShowImages(false)} />}
    </div>
  )
}

function CreateLabModal({
  onClose,
  onCreated,
}: {
  onClose: () => void
  onCreated: (id: string) => void
}) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [busy, setBusy] = useState(false)

  const create = async () => {
    if (!name.trim()) return
    setBusy(true)
    try {
      const lab = await api.createLab({ name: name.trim(), description })
      onCreated(lab.id)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-full max-w-md rounded-xl border border-ink-700 bg-ink-900 p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-lg font-semibold text-white">New Lab</h3>
        <label className="mt-4 block text-xs font-medium uppercase tracking-wide text-ink-400">Name</label>
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void create()}
          placeholder="ospf-triangle"
          className="mt-1 w-full rounded-lg border border-ink-700 bg-ink-950 px-3 py-2 text-sm text-white outline-none focus:border-accent-600"
        />
        <label className="mt-3 block text-xs font-medium uppercase tracking-wide text-ink-400">Description</label>
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          rows={2}
          className="mt-1 w-full rounded-lg border border-ink-700 bg-ink-950 px-3 py-2 text-sm text-white outline-none focus:border-accent-600"
        />
        <div className="mt-5 flex justify-end gap-2">
          <button onClick={onClose} className="rounded-lg px-3 py-1.5 text-sm text-ink-300 hover:text-white">
            Cancel
          </button>
          <button
            onClick={() => void create()}
            disabled={busy || !name.trim()}
            className="rounded-lg px-4 py-1.5 text-sm font-semibold disabled:opacity-50"
            style={{ background: 'var(--accent)', color: '#08211d' }}
          >
            Create
          </button>
        </div>
      </div>
    </div>
  )
}
