import { useEffect } from 'react'
import { useStore } from './store'
import Dashboard from './components/Dashboard'
import LabEditor from './components/LabEditor'
import Login from './components/Login'

export default function App() {
  const view = useStore((s) => s.view)
  const system = useStore((s) => s.system)
  const user = useStore((s) => s.user)
  const authReady = useStore((s) => s.authReady)
  const initAuth = useStore((s) => s.initAuth)
  const loadTemplates = useStore((s) => s.loadTemplates)

  useEffect(() => {
    void initAuth()
  }, [initAuth])

  // Load the template catalog once we're past the auth gate.
  const authed = !system?.auth_enabled || !!user
  useEffect(() => {
    if (authReady && authed) void loadTemplates()
  }, [authReady, authed, loadTemplates])

  if (!authReady) {
    return (
      <div className="flex h-full items-center justify-center text-ink-400">
        <span className="h-5 w-5 animate-spin rounded-full border-2 border-ink-700 border-t-accent-500" />
      </div>
    )
  }

  if (system?.auth_enabled && !user) return <Login />

  return view.kind === 'dashboard' ? <Dashboard /> : <LabEditor key={view.labId} />
}
