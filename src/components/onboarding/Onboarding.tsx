import { useEffect, useState } from 'react'
import { Mic, Keyboard, Key, CheckCircle } from 'lucide-react'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { api } from '../../lib/tauri'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

type Step = 'welcome' | 'permissions' | 'shortcut' | 'apikey' | 'done'

export function Onboarding() {
  const [step, setStep] = useState<Step>('welcome')
  const [apiKey, setApiKey] = useState('')
  const [testState, setTestState] = useState<'idle' | 'loading' | 'ok' | 'fail'>('idle')
  const [permGranted, setPermGranted] = useState(false)
  const [engine, setEngine] = useState<'local' | 'cloud'>('local')

  useEffect(() => {
    api.getSettings().then(s => setEngine(s.engine)).catch(() => {})
  }, [])

  const audioPrivacyCopy = engine === 'local'
    ? 'Your audio is transcribed entirely on your device — never sent to any server.'
    : 'Your audio is sent to your API provider for transcription — never stored by TypeUrVoice.'

  const finish = async () => {
    // Persist FIRST. Everything below is best-effort window plumbing; if the
    // settings save fails we'd rather hear about it than silently re-show
    // onboarding next launch.
    const settings = await api.getSettings()
    await api.updateSettings({ ...settings, onboardingComplete: true })

    // Reach for the pre-declared settings window by label. `new WebviewWindow`
    // with a label already present in tauri.conf.json throws on Tauri 2,
    // which used to drop us into a half-state where the save had landed but
    // the user never saw the settings window open — only the onboarding
    // window hiding, with no follow-up.
    try {
      const existing = await WebviewWindow.getByLabel('settings')
      if (existing) {
        await existing.show()
        await existing.setFocus()
      }
    } catch (e) {
      console.error('Failed to open settings window after onboarding:', e)
    }
    try {
      await getCurrentWindow().hide()
    } catch (e) {
      console.error('Failed to hide onboarding window:', e)
    }
  }

  const testKey = async () => {
    if (!apiKey.trim()) return
    setTestState('loading')
    try {
      const ok = await api.testApiKey('openai', apiKey.trim())
      if (ok) {
        await api.saveApiKey('openai', apiKey.trim())
        setTestState('ok')
      } else {
        setTestState('fail')
      }
    } catch {
      setTestState('fail')
    }
  }

  const requestMicPermission = async () => {
    // On macOS, opening the preference pane will prompt the user
    await api.openPermissionSettings('microphone')
    setPermGranted(true) // Optimistic — user can proceed
  }

  const requestAccessibilityPermission = async () => {
    await api.openPermissionSettings('accessibility')
  }

  return (
    <div style={{
      height: '100vh',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      background: 'var(--bg-primary)',
      padding: 32,
      gap: 0,
    }}>
      {step === 'welcome' && (
        <StepCard
          icon={<div style={{ fontSize: 36 }}>🎙</div>}
          title="Welcome to TypeUrVoice"
          desc="Speak naturally, get perfect text. Press your shortcut, say what you want to write, and TypeUrVoice handles the rest."
          action={<Button variant="primary" size="lg" onClick={() => setStep('permissions')}>Get Started</Button>}
        />
      )}

      {step === 'permissions' && (
        <StepCard
          icon={<Mic size={32} style={{ color: 'var(--text-primary)' }} />}
          title="Microphone access"
          desc={`TypeUrVoice needs microphone access to capture your speech. ${audioPrivacyCopy}`}
          action={
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)', width: '100%', alignItems: 'center' }}>
              {!permGranted ? (
                <Button variant="primary" size="lg" onClick={requestMicPermission}>Grant Microphone Access</Button>
              ) : (
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--accent-green)', fontSize: 'var(--text-sm)' }}>
                  <CheckCircle size={14} /> Granted
                </div>
              )}
              <Button variant="ghost" size="sm" onClick={() => { requestAccessibilityPermission(); setStep('shortcut') }}>
                Also enable accessibility (for auto-paste)
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setStep('shortcut')}>Continue</Button>
            </div>
          }
        />
      )}

      {step === 'shortcut' && (
        <StepCard
          icon={<Keyboard size={32} style={{ color: 'var(--text-primary)' }} />}
          title="Your recording shortcut"
          desc="The default shortcut is Option+Space on macOS and Ctrl+Shift+Space on Windows. You can change it anytime in Settings."
          action={
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)', width: '100%', alignItems: 'center' }}>
              <ShortcutDisplay />
              <Button variant="primary" size="lg" onClick={() => setStep('apikey')}>Looks good</Button>
              <Button variant="ghost" size="sm" onClick={() => setStep('apikey')}>Change later in Settings</Button>
            </div>
          }
        />
      )}

      {step === 'apikey' && (
        <StepCard
          icon={<Key size={32} style={{ color: 'var(--text-primary)' }} />}
          title="Add your OpenAI API key"
          desc="TypeUrVoice uses your own key — no subscription, no data stored. Get your key at platform.openai.com."
          action={
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)', width: '100%', alignItems: 'center' }}>
              <div style={{ width: '100%', display: 'flex', gap: 'var(--sp-2)' }}>
                <div style={{ flex: 1 }}>
                  <Input
                    type="password"
                    placeholder="sk-..."
                    value={apiKey}
                    onChange={e => setApiKey(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && testKey()}
                  />
                </div>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={testKey}
                  loading={testState === 'loading'}
                  disabled={!apiKey.trim()}
                  style={{ height: 30, alignSelf: 'flex-start' }}
                >
                  Test
                </Button>
              </div>
              {testState === 'ok' && (
                <div style={{ display: 'flex', gap: 6, color: 'var(--accent-green)', fontSize: 'var(--text-xs)', alignItems: 'center' }}>
                  <CheckCircle size={12} /> API key saved
                </div>
              )}
              {testState === 'fail' && (
                <div style={{ color: '#FF6B6B', fontSize: 'var(--text-xs)' }}>Invalid key. Check and try again.</div>
              )}
              <Button
                variant="primary"
                size="lg"
                onClick={() => setStep('done')}
              >
                {testState === 'ok' ? 'Continue' : 'Skip for now'}
              </Button>
            </div>
          }
        />
      )}

      {step === 'done' && (
        <StepCard
          icon={<CheckCircle size={32} style={{ color: 'var(--accent-green)' }} />}
          title="You're all set"
          desc="Press your shortcut in any app to start recording. TypeUrVoice runs quietly in the menu bar."
          action={<Button variant="primary" size="lg" onClick={finish}>Start Using TypeUrVoice</Button>}
        />
      )}

      {/* Step dots */}
      <div style={{ display: 'flex', gap: 6, marginTop: 24 }}>
        {(['welcome', 'permissions', 'shortcut', 'apikey', 'done'] as Step[]).map(s => (
          <div
            key={s}
            style={{
              width: s === step ? 16 : 6,
              height: 6,
              borderRadius: 3,
              background: s === step ? 'var(--text-primary)' : 'var(--border-default)',
              transition: 'all var(--t-slow)',
            }}
          />
        ))}
      </div>
    </div>
  )
}

function StepCard({ icon, title, desc, action }: { icon: React.ReactNode; title: string; desc: string; action: React.ReactNode }) {
  return (
    <div style={{
      display: 'flex', flexDirection: 'column', alignItems: 'center',
      gap: 'var(--sp-4)', maxWidth: 380, width: '100%', textAlign: 'center',
      animation: 'fadeIn 250ms ease-out',
    }}>
      {icon}
      <div>
        <h1 style={{ fontSize: 'var(--text-xl)', fontWeight: 600, marginBottom: 8 }}>{title}</h1>
        <p style={{ fontSize: 'var(--text-sm)', color: 'var(--text-secondary)', lineHeight: 1.6 }}>{desc}</p>
      </div>
      {action}
    </div>
  )
}

function ShortcutDisplay() {
  const isMac = navigator.platform.toLowerCase().includes('mac')
  const shortcut = isMac ? ['⌥', 'Space'] : ['Ctrl', 'Shift', 'Space']
  return (
    <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
      {shortcut.map((k, i) => (
        <kbd
          key={i}
          style={{
            padding: '4px 8px', borderRadius: 'var(--radius)',
            border: '1px solid var(--border-default)', background: 'var(--bg-tertiary)',
            fontSize: 'var(--text-sm)', fontFamily: 'inherit',
            color: 'var(--text-primary)',
          }}
        >
          {k}
        </kbd>
      ))}
    </div>
  )
}
