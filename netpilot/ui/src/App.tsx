import { useEffect } from 'react'
import { useStore } from './store'
import Dashboard from './components/Dashboard'
import LabEditor from './components/LabEditor'

export default function App() {
  const view = useStore((s) => s.view)
  const loadTemplates = useStore((s) => s.loadTemplates)

  useEffect(() => {
    void loadTemplates()
  }, [loadTemplates])

  return view.kind === 'dashboard' ? <Dashboard /> : <LabEditor key={view.labId} />
}
