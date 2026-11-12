import { useEffect, useState } from 'react'
import { Input } from '../ui/Input'
import { Select } from '../ui/Select'
import { Toggle } from '../ui/Toggle'
import { Section } from '../ui/Section'
import { RadioCard, RadioCardGroup } from '../ui/RadioCard'
import { LANGUAGES } from '../../lib/constants'
import { api } from '../../lib/tauri'
import type { KnockLanguage, KnockPattern, KnockSensitivity, MediaWhileRecording, Settings } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

const shortcutLangs = LANGUAGES.filter(l => l.code !== 'auto')

export function GeneralTab({ settings, onUpdate }: Props) {
  const [primaryFocused, setPrimaryFocused] = useState(false)
  const [secondaryFocused, setSecondaryFocused] = useState(false)

  const [shortcutError, setShortcutError] = useState<string | null>(null)

  const captureShortcut = (
    e: React.KeyboardEvent<HTMLInputElement>,
    field: 'shortcut' | 'secondaryShortcut',
  ) => {
    e.preventDefault()
    const mods: string[] = []
    if (e.metaKey) mods.push('Meta')
    if (e.ctrlKey) mods.push('Control')
    if (e.altKey) mods.push('Alt')
    if (e.shiftKey) mods.push('Shift')
    const key = e.key
    if (['Meta', 'Control', 'Alt', 'Shift'].includes(key)) return
    const shortcut = [...mods, key.length === 1 ? key.toUpperCase() : key].join('+')

    // Reject conflicts — both shortcuts can't be bound to the same combo.
    const other = field === 'shortcut' ? settings.secondaryShortcut : settings.shortcut
    if (other && other === shortcut) {
      setShortcutError(`"${shortcut}" is already used by the ${field === 'shortcut' ? 'secondary' : 'primary'} shortcut. Pick a different combination.`)
      e.currentTarget.blur()
      return
    }
    setShortcutError(null)
    onUpdate({ [field]: shortcut })
    e.currentTarget.blur()
  }

  const hasSecondary = !!settings.secondaryShortcut

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>
      {shortcutError && (
        <div role="alert" style={{ color: '#FCA5A5', fontSize: 'var(--text-sm)' }}>
          {shortcutError}
        </div>
      )}
      <Section title="Primary Shortcut">
        <div style={{ display: 'flex', gap: 10, alignItems: 'flex-start' }}>
          <div style={{ flex: 1 }}>
            <Input
              label="Shortcut"
              aria-label="Primary recording shortcut"
              value={primaryFocused ? 'Press keys...' : settings.shortcut}
              onFocus={() => setPrimaryFocused(true)}
              onBlur={() => setPrimaryFocused(false)}
              onKeyDown={e => captureShortcut(e, 'shortcut')}
              readOnly={!primaryFocused}
              description="Click to capture a new shortcut"
              style={{ cursor: 'pointer', caretColor: 'transparent' }}
            />
          </div>
          <div style={{ flex: 1 }}>
            <Select
              label="Language"
              value={settings.primaryLanguage}
              onChange={e => onUpdate({ primaryLanguage: e.target.value })}
              options={shortcutLangs.map(l => ({ value: l.code, label: l.label }))}
              description="Language for this shortcut"
            />
          </div>
        </div>
      </Section>

      <Section title="Secondary Shortcut">
        <Toggle
          label="Enable secondary shortcut"
          description="Record in a different language with a second shortcut"
          checked={hasSecondary}
          onChange={val => {
            if (val) {
              onUpdate({ secondaryShortcut: 'Alt+Shift+Space' })
            } else {
              onUpdate({ secondaryShortcut: null })
            }
          }}
        />
        {hasSecondary && (
          <div style={{ display: 'flex', gap: 10, alignItems: 'flex-start', marginTop: 8 }}>
            <div style={{ flex: 1 }}>
              <Input
                label="Shortcut"
                aria-label="Secondary recording shortcut"
                value={secondaryFocused ? 'Press keys...' : (settings.secondaryShortcut ?? '')}
                onFocus={() => setSecondaryFocused(true)}
                onBlur={() => setSecondaryFocused(false)}
                onKeyDown={e => captureShortcut(e, 'secondaryShortcut')}
                readOnly={!secondaryFocused}
                description="Click to capture a new shortcut"
                style={{ cursor: 'pointer', caretColor: 'transparent' }}
              />
            </div>
            <div style={{ flex: 1 }}>
              <Select
                label="Language"
                value={settings.secondaryLanguage}
                onChange={e => onUpdate({ secondaryLanguage: e.target.value })}
                options={shortcutLangs.map(l => ({ value: l.code, label: l.label }))}
                description="Language for this shortcut"
              />
            </div>
          </div>
        )}
      </Section>

      <Section title="Microphone">
        <MicrophonePicker
          value={settings.inputDevice}
          onChange={(v) => onUpdate({ inputDevice: v })}
        />
      </Section>

      <Section title="Recording">
        <RadioCardGroup label="Recording mode">
          <RadioCard
            variant="native"
            label="Hold to Record"
            description="Hold shortcut to record, release to stop. Best for quick messages."
            checked={settings.recordingMode === 'hold'}
            onChange={() => onUpdate({ recordingMode: 'hold' })}
          />
          <RadioCard
            variant="native"
            label="Toggle (Push-to-Start)"
            description="Press once to start, press again to stop. Best for longer dictation."
            checked={settings.recordingMode === 'toggle'}
            onChange={() => onUpdate({ recordingMode: 'toggle' })}
          />
        </RadioCardGroup>
        <Toggle
          label="Show live text while speaking"
          description={settings.engine === 'local'
            ? 'Your words appear in the recording bubble as you talk.'
            : 'Available with the Local engine (Settings → Engine).'}
          checked={settings.livePreview}
          onChange={val => onUpdate({ livePreview: val })}
        />
        <Toggle
          label="Type sentences while speaking"
          description={
            settings.engine !== 'local'
              ? 'Available with the Local engine (Settings → Engine).'
              : !settings.autoPaste
                ? 'Needs Auto-paste (Settings → Output).'
                : settings.summaryEnabled
                  ? 'Paused while Summary is on — a summary needs the whole dictation.'
                  : settings.outputMode === 'markdown'
                    ? 'Paused in Markdown mode — the document is built from the whole dictation.'
                  : 'Each finished sentence goes into your text field (cleaned up and translated) while you keep talking. The rest follows when you stop.'
          }
          checked={settings.streamTyping}
          onChange={val => onUpdate({ streamTyping: val })}
        />
      </Section>

      <MediaSection settings={settings} onUpdate={onUpdate} />

      <KnockSection settings={settings} onUpdate={onUpdate} />

      <Section title="System">
        <Toggle
          label="Start on login"
          description="Launch TypeUrVoice when you log in"
          checked={settings.startOnLogin}
          onChange={val => onUpdate({ startOnLogin: val })}
        />
        <UpdatesRow />
        <Toggle
          label="Verbose diagnostics"
          description="Include transcript previews in crash.log for debugging. Off by default to protect privacy."
          checked={settings.diagnosticsVerbose}
          onChange={val => onUpdate({ diagnosticsVerbose: val })}
        />
        <CrashLogRow />
      </Section>
    </div>
  )
}

const MEDIA_HINTS: Record<MediaWhileRecording, string> = {
  off: 'Sound keeps playing while you dictate.',
  mute: 'Mutes your speakers or headphones while you record and unmutes them when you stop. Videos keep playing silently.',
  pause: 'If something is playing — a video, music, a podcast — it pauses while you record and resumes when you stop. Uses the Play/Pause key, so it needs Accessibility (like auto-paste).',
}

/** Only rendered where it's implemented (macOS). */
function MediaSection({ settings, onUpdate }: Props) {
  const [available, setAvailable] = useState(false)

  useEffect(() => {
    api.mediaControlAvailable()
      .then(setAvailable)
      .catch(() => setAvailable(false))
  }, [])

  if (!available) return null

  return (
    <Section title="Sound While Recording">
      <Select
        label="When recording starts"
        value={settings.mediaWhileRecording}
        onChange={e => onUpdate({ mediaWhileRecording: e.target.value as MediaWhileRecording })}
        options={[
          { value: 'off', label: 'Do nothing' },
          { value: 'pause', label: 'Pause videos and music' },
          { value: 'mute', label: 'Mute sound' },
        ]}
        description={MEDIA_HINTS[settings.mediaWhileRecording]}
      />
    </Section>
  )
}

/** Only rendered on Macs with the built-in accelerometer (Apple Silicon MacBooks). */
function KnockSection({ settings, onUpdate }: Props) {
  const [available, setAvailable] = useState(false)

  useEffect(() => {
    api.knockAvailable()
      .then(setAvailable)
      .catch(() => setAvailable(false))
  }, [])

  if (!available) return null

  const langName = (code: string) => LANGUAGES.find(l => l.code === code)?.label ?? code
  const primary = langName(settings.primaryLanguage)
  const secondary = langName(settings.secondaryLanguage)
  const split = settings.knockLanguage === 'split'
  const knockLangHints: Record<KnockLanguage, string> = {
    primary: `Records in ${primary}. Speaking another language gives a ${primary} result (translated).`,
    secondary: `Records in ${secondary}. Speaking another language gives a ${secondary} result (translated).`,
    split: `Knock twice for ${primary}, three times for ${secondary}. Either pattern stops the recording. After a double knock it waits a moment for a third, so recording starts about 0.5 s after your last knock.`,
    auto: 'Whatever language you speak is what you get — no translation. Short clips can be misdetected (Czech vs Slovak especially); pick a fixed language if that happens.',
  }

  return (
    <Section title="Knock to Record">
      <Toggle
        label="Knock on the laptop to start and stop recording"
        description="Knock on the case, no keys needed. Typing and clicking are ignored."
        checked={settings.knockEnabled}
        onChange={val => onUpdate({ knockEnabled: val })}
      />
      {settings.knockEnabled && (
        <div style={{ marginTop: 8 }}>
          <Select
            label="Language"
            value={settings.knockLanguage}
            onChange={e => onUpdate({ knockLanguage: e.target.value as KnockLanguage })}
            options={[
              { value: 'primary', label: `${primary} (primary shortcut)` },
              { value: 'secondary', label: `${secondary} (secondary shortcut)` },
              { value: 'split', label: `Double knock = ${primary}, triple knock = ${secondary}` },
              { value: 'auto', label: 'Detect the language I speak' },
            ]}
            description={knockLangHints[settings.knockLanguage]}
          />
        </div>
      )}
      {settings.knockEnabled && (
        <div style={{ display: 'flex', gap: 10, alignItems: 'flex-start', marginTop: 8 }}>
          {!split && <div style={{ flex: 1 }}>
            <Select
              label="Knocks"
              value={settings.knockPattern}
              onChange={e => onUpdate({ knockPattern: e.target.value as KnockPattern })}
              options={[
                { value: 'double', label: 'Double knock' },
                { value: 'triple', label: 'Triple knock' },
              ]}
              description="Use Triple if another app already reacts to double knocks."
            />
          </div>}
          <div style={{ flex: 1 }}>
            <Select
              label="Sensitivity"
              value={settings.knockSensitivity}
              onChange={e => onUpdate({ knockSensitivity: e.target.value as KnockSensitivity })}
              options={[
                { value: 'low', label: 'Low' },
                { value: 'medium', label: 'Medium' },
                { value: 'high', label: 'High' },
              ]}
              description="Raise it if gentle knocks are missed; lower it if recording starts by itself."
            />
          </div>
        </div>
      )}
    </Section>
  )
}

function MicrophonePicker({ value, onChange }: {
  value: string | null
  onChange: (v: string | null) => void
}) {
  const [devices, setDevices] = useState<string[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    api.listInputDevices()
      .then(setDevices)
      .catch(() => setDevices([]))
      .finally(() => setLoading(false))
  }, [])

  const isMissing = value !== null && !loading && !devices.includes(value)

  return (
    <div>
      <Select
        label="Input device"
        value={value ?? ''}
        onChange={e => onChange(e.target.value === '' ? null : e.target.value)}
        options={[
          { value: '', label: 'System default' },
          ...devices.map(name => ({ value: name, label: name })),
          ...(isMissing ? [{ value: value!, label: `${value} (not connected)` }] : []),
        ]}
        description={
          loading
            ? 'Loading available devices…'
            : 'Pick a specific microphone or use the system default.'
        }
      />
      {isMissing && (
        <div role="alert" style={{ color: '#FCA5A5', fontSize: 'var(--text-xs)', marginTop: 4 }}>
          The saved device is not currently connected. TypeUrVoice will fall back to the system default.
        </div>
      )}
    </div>
  )
}

function CrashLogRow() {
  const [error, setError] = useState<string | null>(null)
  const open = async () => {
    setError(null)
    try {
      await api.showCrashLog()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', marginTop: 'var(--sp-3)' }}>
      <button
        type="button"
        onClick={open}
        style={{
          padding: '6px 12px',
          background: 'transparent',
          color: 'var(--text-primary)',
          border: '1px solid var(--border-default)',
          borderRadius: 'var(--radius)',
          cursor: 'pointer',
          fontSize: 'var(--text-sm)',
        }}
      >
        Show crash.log
      </button>
      {error && (
        <span role="alert" style={{ color: '#FCA5A5', fontSize: 'var(--text-sm)' }}>
          {error}
        </span>
      )}
    </div>
  )
}

function UpdatesRow() {
  const [checking, setChecking] = useState(false)
  const [result, setResult] = useState<string | null>(null)

  const check = async () => {
    setChecking(true)
    setResult(null)
    try {
      const r = await api.checkForUpdates()
      if (r.available) {
        setResult(`Update available: v${r.version}`)
      } else {
        setResult('You are on the latest version.')
      }
    } catch (e) {
      setResult(`Check failed: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setChecking(false)
    }
  }

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', marginTop: 'var(--sp-3)' }}>
      <button
        type="button"
        onClick={check}
        disabled={checking}
        style={{
          padding: '6px 12px',
          background: 'transparent',
          color: 'var(--text-primary)',
          border: '1px solid var(--border-default)',
          borderRadius: 'var(--radius)',
          cursor: checking ? 'wait' : 'pointer',
          fontSize: 'var(--text-sm)',
        }}
      >
        {checking ? 'Checking…' : 'Check for updates'}
      </button>
      {result && (
        <span style={{ color: 'var(--text-tertiary)', fontSize: 'var(--text-sm)' }} role="status">
          {result}
        </span>
      )}
    </div>
  )
}

