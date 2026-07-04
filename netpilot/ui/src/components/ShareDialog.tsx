// Lab sharing dialog: toggle public/private visibility and manage per-user
// view/edit grants. Only rendered when persistence is enabled.

import { useEffect, useState } from 'react'
import { Globe, Lock, Trash2, X } from 'lucide-react'
import { api, type LabShares } from '../api'
import { useStore } from '../store'

export default function ShareDialog({ labId, onClose }: { labId: string; onClose: () => void }) {
  const pushLog = useStore((s) => s.pushLog)
  const [shares, setShares] = useState<LabShares | null>(null)
  const [username, setUsername] = useState('')
  const [access, setAccess] = useState<'view' | 'edit'>('view')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const refresh = async () => {
    try {
      setShares(await api.labShares(labId))
    } catch (e) {
      setError(e instanceof Error ? e.message : 'failed to load sharing')
    }
  }
  useEffect(() => {
    void refresh()
  }, [labId]) // eslint-disable-line react-hooks/exhaustive-deps

  const setVisibility = async (visibility: 'private' | 'public') => {
    try {
      setShares(await api.shareLab(labId, { visibility }))
    } catch (e) {
      setError(e instanceof Error ? e.message : 'failed')
    }
  }

  const addGrant = async () => {
    if (!username.trim() || busy) return
    setBusy(true)
    setError(null)
    try {
      setShares(await api.shareLab(labId, { username: username.trim(), access }))
      setUsername('')
      pushLog('info', `shared with ${username.trim()} (${access})`)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'failed to share')
    } finally {
      setBusy(false)
    }
  }

  const revoke = async (name: string) => {
    try {
      await api.unshareLab(labId, name)
      await refresh()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'failed to revoke')
    }
  }

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-full max-w-md rounded-2xl border border-ink-700 bg-ink-900 p-5"
        onClick={(e) => e.stopPropagation()}
        style={{ boxShadow: 'var(--shadow)' }}
      >
        <div className="flex items-center gap-2">
          <h3 className="text-base font-semibold text-white">Share lab</h3>
          <button onClick={onClose} className="ml-auto rounded p-1 text-ink-400 hover:text-white">
            <X size={16} />
          </button>
        </div>
        {shares?.owner && (
          <p className="mt-0.5 text-xs text-ink-500">Owned by {shares.owner}</p>
        )}

        {/* visibility */}
        <div className="mt-4 flex gap-2">
          {(['private', 'public'] as const).map((v) => {
            const active = shares?.visibility === v
            const Icon = v === 'public' ? Globe : Lock
            return (
              <button
                key={v}
                onClick={() => void setVisibility(v)}
                className={`flex flex-1 items-center gap-2 rounded-lg border px-3 py-2 text-sm ${
                  active
                    ? 'border-accent-600 bg-accent-600/15 text-accent-500'
                    : 'border-ink-700 text-ink-300 hover:border-ink-600'
                }`}
              >
                <Icon size={14} />
                <span className="capitalize">{v}</span>
                <span className="ml-auto text-[10px] text-ink-500">
                  {v === 'public' ? 'any user can view' : 'invite only'}
                </span>
              </button>
            )
          })}
        </div>

        {/* add grant */}
        <div className="mt-4 flex gap-2">
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void addGrant()}
            placeholder="username to invite"
            className="flex-1 rounded-lg border border-ink-700 bg-ink-950 px-3 py-2 text-sm text-white outline-none focus:border-accent-600"
          />
          <select
            value={access}
            onChange={(e) => setAccess(e.target.value as 'view' | 'edit')}
            className="rounded-lg border border-ink-700 bg-ink-950 px-2 text-sm text-ink-200 outline-none"
          >
            <option value="view">can view</option>
            <option value="edit">can edit</option>
          </select>
          <button
            onClick={() => void addGrant()}
            disabled={busy || !username.trim()}
            className="rounded-lg px-3 text-sm font-semibold text-[#08211d] disabled:opacity-40"
            style={{ background: 'var(--accent)' }}
          >
            Add
          </button>
        </div>

        {error && <div className="mt-3 text-xs text-[#f0655a]">{error}</div>}

        {/* grants list */}
        <div className="mt-4 space-y-1">
          {shares?.shares.length === 0 && (
            <p className="text-xs text-ink-500">No one else has access yet.</p>
          )}
          {shares?.shares.map((g) => (
            <div key={g.user_id} className="flex items-center gap-2 rounded-lg bg-ink-850 px-3 py-2">
              <span
                className="flex h-6 w-6 items-center justify-center rounded-full bg-ink-700 text-[10px] font-semibold text-ink-200"
              >
                {g.username.slice(0, 2).toUpperCase()}
              </span>
              <span className="text-sm text-ink-200">{g.username}</span>
              <span className="ml-auto rounded bg-ink-800 px-1.5 py-0.5 text-[10px] uppercase text-ink-400">
                {g.access}
              </span>
              <button
                onClick={() => void revoke(g.username)}
                className="rounded p-1 text-ink-400 hover:bg-red-900/40 hover:text-red-300"
                title="Revoke"
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
