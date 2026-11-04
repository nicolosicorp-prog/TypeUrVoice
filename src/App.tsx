import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { SettingsWindow } from './components/settings/SettingsWindow'
import { RecordingOverlay } from './components/overlay/RecordingOverlay'
import { Onboarding } from './components/onboarding/Onboarding'
import './styles/globals.css'

type WindowLabel = 'settings' | 'overlay' | 'onboarding'

export default function App() {
  const [label, setLabel] = useState<WindowLabel | null>(null)

  useEffect(() => {
    const win = getCurrentWindow()
    const l = win.label as WindowLabel
    setLabel(l)

    // Overlay window needs a fully transparent background
    if (l === 'overlay') {
      document.documentElement.classList.add('overlay-window')
    }
  }, [])

  if (!label) return null

  return (
    <>
      {label === 'settings' && <SettingsWindow />}
      {label === 'overlay' && <RecordingOverlay />}
      {label === 'onboarding' && <Onboarding />}
    </>
  )
}
