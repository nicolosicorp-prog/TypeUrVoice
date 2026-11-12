import { useState, useEffect, useCallback, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { api } from '../lib/tauri'
import type { Settings } from '../types'

const DEFAULT_SETTINGS: Settings = {
  onboardingComplete: false,
  shortcut: 'Option+Space',
  recordingMode: 'hold',
  engine: 'cloud',
  localModel: 'turbo-q5',
  outputMode: 'clean',
  autoPaste: true,
  showNotification: true,
  translationEnabled: false,
  sourceLanguage: 'auto',
  targetLanguage: 'en',
  summaryEnabled: false,
  summaryType: 'keyPoints',
  summaryShortcut: null,
  historyRetentionDays: 30,
  startOnLogin: false,
  dictionary: [],
  processingModel: 'gpt-6-luna',
  textProcessor: 'openai',
  textProcessorUrl: 'http://localhost:11434',
  textProcessorModel: 'gemma4',
  secondaryShortcut: null,
  secondaryLanguage: 'en',
  primaryLanguage: 'cs',
  diagnosticsVerbose: false,
  inputDevice: null,
  customPrompt: '',
  customProfiles: [],
  activeCustomProfileId: null,
  openaiCompatPreset: 'grok',
  openaiCompatUrl: '',
  cloudTranscriptionProvider: 'openai',
  cloudTranscriptionModel: null,
  appLocale: 'en',
  activeOccupationProfileId: null,
  userOccupationProfiles: [],
  localeFormatEnabled: true,
  removeFillers: true,
  livePreview: true,
  streamTyping: false,
  speakerNotes: '',
  learnFromHistory: true,
  knockEnabled: false,
  knockPattern: 'double',
  knockSensitivity: 'medium',
  knockLanguage: 'primary',
  mediaWhileRecording: 'off',
  markdownTemplate: '',
  markdownLanguage: 'auto',
  builtinLlmModel: 'qwen3.5-2b',
  builtinLlmAuto: true,
  builtinLlmIdleMinutes: 5,
  privacyMode: false,
  privacyModeSnapshot: null,
}

export function useSettings() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS)
  const [loading, setLoading] = useState(true)
  const [lastError, setLastError] = useState<string | null>(null)
  // Mirror of `settings` for the update() callback. Avoids writing back stale
  // DEFAULT_SETTINGS (with onboardingComplete=false) if anything triggers an
  // update before the initial getSettings() promise resolves.
  const latestRef = useRef<Settings>(DEFAULT_SETTINGS)
  const loadedRef = useRef(false)

  const applySettings = useCallback((s: Settings) => {
    latestRef.current = s
    setSettings(s)
  }, [])

  useEffect(() => {
    api.getSettings()
      .then(s => { loadedRef.current = true; applySettings(s) })
      .catch(console.error)
      .finally(() => setLoading(false))
  }, [applySettings])

  // Listen for external settings changes (from tray menu)
  useEffect(() => {
    let unlisten: (() => void) | undefined
    listen('settings-changed', () => {
      api.getSettings().then(applySettings).catch(console.error)
    }).then(fn => { unlisten = fn })
    return () => { unlisten?.() }
  }, [applySettings])

  const update = useCallback(async (patch: Partial<Settings>) => {
    // Refuse writes until the initial load completes. Otherwise the merge
    // base is DEFAULT_SETTINGS (onboardingComplete=false), and writing that
    // back to disk silently reverts onboarding state for the next launch.
    if (!loadedRef.current) {
      console.warn('useSettings.update() called before initial load — ignoring patch:', patch)
      return
    }
    const next = { ...latestRef.current, ...patch }
    applySettings(next)
    try {
      await api.updateSettings(next)
    } catch (e) {
      console.error('Failed to save settings:', e)
      setLastError(typeof e === 'string' ? e : (e as Error)?.message ?? 'Failed to save settings')
      // Revert on failure
      api.getSettings().then(applySettings).catch(console.error)
    }
  }, [applySettings])

  const clearError = useCallback(() => setLastError(null), [])

  return { settings, update, loading, lastError, clearError }
}
