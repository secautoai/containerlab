// Image library manager: list base images, upload new ones.

import { useEffect, useRef, useState } from 'react'
import { HardDrive, Upload, X } from 'lucide-react'
import { api, type DiskImage } from '../api'
import { useStore } from '../store'

function human(bytes: number): string {
  if (bytes > 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GiB`
  if (bytes > 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(0)} MiB`
  return `${(bytes / 1024).toFixed(0)} KiB`
}

export default function ImageManager({ onClose }: { onClose: () => void }) {
  const templates = useStore((s) => s.templates)
  const loadTemplates = useStore((s) => s.loadTemplates)
  const [images, setImages] = useState<DiskImage[]>([])
  const [template, setTemplate] = useState('')
  const [version, setVersion] = useState('')
  const [progress, setProgress] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const refresh = async () => setImages(await api.images())
  useEffect(() => {
    void refresh()
  }, [])

  const upload = (file: File) => {
    if (!template || !version.trim()) {
      setError('pick a template and enter a version first')
      return
    }
    setError(null)
    setProgress(0)
    const xhr = new XMLHttpRequest()
    xhr.open(
      'PUT',
      `/api/images/${encodeURIComponent(template)}/${encodeURIComponent(version.trim())}/${encodeURIComponent(file.name)}`,
    )
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) setProgress(Math.round((e.loaded / e.total) * 100))
    }
    xhr.onload = () => {
      setProgress(null)
      if (xhr.status >= 200 && xhr.status < 300) {
        void refresh()
        void loadTemplates() // refresh available_images in palette
      } else {
        try {
          setError(JSON.parse(xhr.responseText).error)
        } catch {
          setError(`upload failed (${xhr.status})`)
        }
      }
    }
    xhr.onerror = () => {
      setProgress(null)
      setError('upload failed (network)')
    }
    xhr.send(file)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="max-h-[80vh] w-full max-w-2xl overflow-y-auto rounded-xl border border-ink-700 bg-ink-900 p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2">
          <HardDrive size={18} className="text-accent-500" />
          <h3 className="text-lg font-semibold text-white">Image Library</h3>
          <button onClick={onClose} className="ml-auto rounded p-1 text-ink-400 hover:bg-ink-700 hover:text-white">
            <X size={16} />
          </button>
        </div>
        <p className="mt-1 text-xs text-ink-500">
          Base images are immutable; nodes boot copy-on-write overlays. Layout:{' '}
          <code className="text-ink-400">images/&lt;template&gt;/&lt;version&gt;/file.qcow2</code>
        </p>

        <div className="mt-4 flex items-end gap-2 rounded-lg border border-ink-800 bg-ink-950 p-3">
          <label className="block flex-1">
            <span className="text-[10px] font-medium uppercase tracking-wide text-ink-400">Template</span>
            <select
              value={template}
              onChange={(e) => setTemplate(e.target.value)}
              className="mt-0.5 w-full rounded-md border border-ink-700 bg-ink-900 px-2 py-1.5 text-sm text-white"
            >
              <option value="">choose…</option>
              {templates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block flex-1">
            <span className="text-[10px] font-medium uppercase tracking-wide text-ink-400">Version label</span>
            <input
              value={version}
              onChange={(e) => setVersion(e.target.value)}
              placeholder="1.5 / 17.12.01"
              className="mt-0.5 w-full rounded-md border border-ink-700 bg-ink-900 px-2 py-1.5 text-sm text-white"
            />
          </label>
          <button
            onClick={() => fileRef.current?.click()}
            disabled={progress !== null}
            className="flex items-center gap-1.5 rounded-md bg-accent-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-accent-500 disabled:opacity-50"
          >
            <Upload size={14} />
            {progress !== null ? `${progress}%` : 'Upload'}
          </button>
          <input
            ref={fileRef}
            type="file"
            accept=".qcow2,.img,.iso,.vmdk"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0]
              if (f) upload(f)
              e.target.value = ''
            }}
          />
        </div>
        {error && <p className="mt-2 text-xs text-red-400">{error}</p>}

        <table className="mt-4 w-full text-left text-sm">
          <thead>
            <tr className="text-[10px] uppercase tracking-wider text-ink-500">
              <th className="pb-1 font-medium">Template</th>
              <th className="pb-1 font-medium">Version</th>
              <th className="pb-1 font-medium">File</th>
              <th className="pb-1 text-right font-medium">Size</th>
            </tr>
          </thead>
          <tbody>
            {images.map((img) => (
              <tr key={img.path} className="border-t border-ink-800 text-ink-300">
                <td className="py-1.5 font-mono text-xs">{img.template}</td>
                <td className="py-1.5 font-mono text-xs">{img.version}</td>
                <td className="py-1.5 truncate text-xs text-ink-500">{img.path.split('/').pop()}</td>
                <td className="py-1.5 text-right text-xs">{human(img.size_bytes)}</td>
              </tr>
            ))}
            {images.length === 0 && (
              <tr>
                <td colSpan={4} className="py-6 text-center text-xs text-ink-600">
                  No images yet — upload a qcow2 to light up the palette.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
