import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Toggle } from '../ui/Toggle'
import { Button } from '../ui/Button'
import { Section } from '../ui/Section'
import { api } from '../../lib/tauri'
import type { Settings, SpeakerProfile, TextProcessor } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

const ON_DEVICE: TextProcessor[] = ['builtin', 'ollama', 'lmstudio']

const areaStyle: React.CSSProperties = {
  width: '100%',
  minHeight: 54,
  background: 'var(--bg-secondary)',
  color: 'var(--text-primary)',
  border: '1px solid var(--border-default)',
  borderRadius: 'var(--radius)',
  padding: '6px 8px',
  fontSize: 'var(--text-xs)',
  fontFamily: 'inherit',
  lineHeight: 1.5,
  resize: 'vertical',
  boxSizing: 'border-box',
}

const labelStyle: React.CSSProperties = {
  fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', fontWeight: 500, marginBottom: 4,
}

const EMPTY: SpeakerProfile = { topics: '', terms: [], recognition: '', style: '', updatedAt: 0, lastHistoryId: 0 }

/** "How you speak": the user's own note plus the profile TypeUrVoice learns
 *  from History. Both feed cleanup, translation and Whisper's vocabulary. */
export function YourVoicePanel({ settings, onUpdate }: Props) {
  const [notes, setNotes] = useState(settings.speakerNotes)
  const [profile, setProfile] = useState<SpeakerProfile>(EMPTY)
  const [termsText, setTermsText] = useState('')
  const [dirty, setDirty] = useState(false)
  const [busy, setBusy] = useState(false)
  const [status, setStatus] = useState<string | null>(null)

  useEffect(() => { setNotes(settings.speakerNotes) }, [settings.speakerNotes])

  const showProfile = (p: SpeakerProfile) => {
    setProfile(p)
    setTermsText(p.terms.join(', '))
    setDirty(false)
  }

  useEffect(() => {
    api.getSpeakerProfile().then(showProfile).catch(console.error)
    const un = listen<SpeakerProfile>('speaker-profile-updated', e => {
      // Don't clobber edits the user hasn't saved yet.
      setDirty(d => { if (!d) showProfile(e.payload); return d })
    })
    return () => { un.then(f => f()) }
  }, [])

  const edit = (patch: Partial<SpeakerProfile>) => { setProfile(p => ({ ...p, ...patch })); setDirty(true) }

  const save = async () => {
    const next = { ...profile, terms: termsText.split(',').map(t => t.trim()).filter(Boolean) }
    try {
      await api.saveSpeakerProfile(next)
      showProfile(await api.getSpeakerProfile())
      setStatus('Saved.')
    } catch (e) {
      setStatus(`Couldn't save: ${e}`)
    }
  }

  const learnNow = async () => {
    setBusy(true)
    setStatus('Reading your recent dictations…')
    try {
      showProfile(await api.learnSpeakerProfileNow())
      setStatus('Updated from your recent dictations.')
    } catch (e) {
      setStatus(`Update failed: ${e}`)
    } finally {
      setBusy(false)
    }
  }

  const clear = async () => {
    await api.clearSpeakerProfile()
    showProfile(EMPTY)
    setStatus('Cleared. Learning starts again from your next dictations.')
  }

  const onDevice = ON_DEVICE.includes(settings.textProcessor)
  const updated = profile.updatedAt
    ? new Date(profile.updatedAt * 1000).toLocaleString()
    : null

  return (
    <Section title="How You Speak">
      <div>
        <div style={labelStyle}>Notes about your speech</div>
        <textarea
          value={notes}
          maxLength={600}
          onChange={e => setNotes(e.target.value)}
          onBlur={() => notes !== settings.speakerNotes && onUpdate({ speakerNotes: notes })}
          placeholder="e.g. I can't pronounce ř — it sounds like ž or r. I often mix in English tech words."
          aria-label="Notes about your speech"
          style={areaStyle}
        />
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 2 }}>
          Cleanup and translation use this to fix words the recognizer gets wrong for your voice.
        </div>
      </div>

      <Toggle
        label="Learn from my dictations"
        description={onDevice
          ? 'Every few dictations, your on-device model updates a short profile: your topics, names, words it mishears and how you phrase things.'
          : 'Your text processor is online, so the profile only updates when you click "Update now" (that sends recent dictations to it).'}
        checked={settings.learnFromHistory}
        onChange={val => onUpdate({ learnFromHistory: val })}
      />

      {settings.learnFromHistory && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)' }}>
          <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)' }}>
            {updated ? `Learned profile · updated ${updated}` : 'Nothing learned yet.'} You can edit it.
          </div>
          <div>
            <div style={labelStyle}>Words it mishears for you</div>
            <textarea value={profile.recognition} onChange={e => edit({ recognition: e.target.value })}
              aria-label="Words it mishears for you" style={areaStyle} />
          </div>
          <div>
            <div style={labelStyle}>Names and terms (comma-separated)</div>
            <textarea value={termsText} onChange={e => { setTermsText(e.target.value); setDirty(true) }}
              aria-label="Names and terms" style={{ ...areaStyle, minHeight: 40 }} />
          </div>
          <div>
            <div style={labelStyle}>Topics</div>
            <textarea value={profile.topics} onChange={e => edit({ topics: e.target.value })}
              aria-label="Topics" style={{ ...areaStyle, minHeight: 40 }} />
          </div>
          <div>
            <div style={labelStyle}>Your style (used for translations)</div>
            <textarea value={profile.style} onChange={e => edit({ style: e.target.value })}
              aria-label="Your style" style={areaStyle} />
          </div>
          <div style={{ display: 'flex', gap: 'var(--sp-2)', alignItems: 'center', flexWrap: 'wrap' }}>
            <Button variant="secondary" size="sm" onClick={save} disabled={!dirty}>Save edits</Button>
            <Button variant="secondary" size="sm" onClick={learnNow} disabled={busy}>
              {busy ? 'Updating…' : 'Update now'}
            </Button>
            <Button variant="ghost" size="sm" onClick={clear} disabled={busy}>Clear</Button>
            {status && (
              <span aria-live="polite" style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)' }}>{status}</span>
            )}
          </div>
        </div>
      )}
    </Section>
  )
}
