// Login gate shown when the server has multi-user persistence enabled.

import { useState } from 'react'
import { useStore } from '../store'
import { stratoMark } from '../vendors'

const grotesk = "'Space Grotesk', 'IBM Plex Sans', sans-serif"

export default function Login() {
  const login = useStore((s) => s.login)
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    if (!username.trim() || !password || busy) return
    setBusy(true)
    setError(null)
    try {
      await login(username.trim(), password)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'login failed')
      setBusy(false)
    }
  }

  return (
    <div style={{ height: '100%', display: 'grid', placeItems: 'center', background: 'var(--bg)' }}>
      <div
        style={{
          width: 340,
          background: 'var(--panel)',
          border: '1px solid var(--border)',
          borderRadius: 16,
          padding: '28px 26px',
          boxShadow: 'var(--shadow)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 4 }}>
          <div style={{ width: 32, height: 32, borderRadius: 9, background: 'linear-gradient(135deg,var(--accent),#2a8fd1)', display: 'grid', placeItems: 'center' }}>
            {stratoMark({ size: 17 })}
          </div>
          <span style={{ fontFamily: grotesk, fontWeight: 700, fontSize: 19, letterSpacing: '-.3px' }}>strato</span>
        </div>
        <div style={{ fontSize: 12.5, color: 'var(--muted)', marginBottom: 20 }}>
          Sign in to your network lab workspace.
        </div>

        <label style={{ fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.4px', color: 'var(--muted)' }}>
          Username
        </label>
        <input
          autoFocus
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && document.getElementById('login-pw')?.focus()}
          style={inputStyle}
        />
        <label style={{ fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.4px', color: 'var(--muted)', marginTop: 12, display: 'block' }}>
          Password
        </label>
        <input
          id="login-pw"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && submit()}
          style={inputStyle}
        />

        {error && (
          <div style={{ marginTop: 12, fontSize: 12, color: 'var(--red)', background: 'rgba(240,101,90,.08)', border: '1px solid rgba(240,101,90,.3)', borderRadius: 8, padding: '8px 10px' }}>
            {error}
          </div>
        )}

        <button
          onClick={submit}
          disabled={busy || !username.trim() || !password}
          style={{
            width: '100%',
            marginTop: 18,
            border: 'none',
            background: busy || !username.trim() || !password ? 'var(--border2)' : 'var(--accent)',
            color: '#08211d',
            borderRadius: 10,
            padding: '10px 0',
            fontSize: 13.5,
            fontWeight: 700,
            cursor: busy ? 'default' : 'pointer',
          }}
        >
          {busy ? 'Signing in…' : 'Sign in'}
        </button>
      </div>
    </div>
  )
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  marginTop: 5,
  background: 'var(--panel2)',
  border: '1px solid var(--border2)',
  borderRadius: 9,
  padding: '9px 11px',
  fontSize: 13,
  color: 'var(--text)',
  outline: 'none',
  fontFamily: 'inherit',
}
