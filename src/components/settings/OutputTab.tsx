import { useState, useRef, useEffect, useMemo } from 'react'
import { Plus, Trash2, Pencil, Check, Briefcase } from 'lucide-react'
import { Toggle } from '../ui/Toggle'
import { Section } from '../ui/Section'
import { RadioCard, RadioCardGroup } from '../ui/RadioCard'
import { Button } from '../ui/Button'
import { api } from '../../lib/tauri'
import type { Settings, OutputMode, CustomProfile, OccupationProfile, AppLocale, Occupation } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

const MODES: { value: OutputMode; label: string; desc: string }[] = [
  { value: 'raw',      label: 'Raw',      desc: 'Exact transcription. No cleanup.' },
  { value: 'clean',    label: 'Clean',    desc: 'Punctuation, grammar, filler removal.' },
  { value: 'polished', label: 'Polished', desc: 'Rewritten into clear, professional prose.' },
  { value: 'custom',   label: 'Custom',   desc: 'Apply one of your saved profiles to every transcription.' },
]

const MAX_PROMPT_CHARS = 3000
const MAX_NAME_CHARS = 60

export function OutputTab({ settings, onUpdate }: Props) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>
      <Section title="Professional Profiles">
        <ProfessionalProfiles settings={settings} onUpdate={onUpdate} />
      </Section>

      <Section title="Output Mode">
        <RadioCardGroup label="Output Mode">
          {MODES.map(mode => (
            <RadioCard
              key={mode.value}
              label={mode.label}
              description={mode.desc}
              checked={settings.outputMode === mode.value}
              onChange={() => onUpdate({ outputMode: mode.value })}
            />
          ))}
        </RadioCardGroup>
      </Section>

      {settings.outputMode === 'custom' && (
        <Section title="Custom Profiles">
          <ProfileManager settings={settings} onUpdate={onUpdate} />
        </Section>
      )}

      <Section title="Behavior">
        <Toggle
          label="Remove filler words"
          description="Drop um, uh, ehm, eee, hmm… from every transcript — works in Raw mode too, no AI needed"
          checked={settings.removeFillers}
          onChange={val => onUpdate({ removeFillers: val })}
        />
        <Toggle
          label="Auto-paste"
          description="Paste result into the active text field"
          checked={settings.autoPaste}
          onChange={val => onUpdate({ autoPaste: val })}
        />
        <Toggle
          label="Show notification"
          description="Preview transcription after processing"
          checked={settings.showNotification}
          onChange={val => onUpdate({ showNotification: val })}
        />
      </Section>
    </div>
  )
}

// ─────────────────────────────────────────────────────────────────────────────
// Profile Manager
// ─────────────────────────────────────────────────────────────────────────────

function ProfileManager({ settings, onUpdate }: Props) {
  const profiles = settings.customProfiles
  const activeId = settings.activeCustomProfileId

  // Track which profile is being renamed (id) — null when no rename in progress
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const renameInputRef = useRef<HTMLInputElement>(null)

  // Focus the rename input when it opens
  useEffect(() => {
    if (renamingId && renameInputRef.current) {
      renameInputRef.current.focus()
      renameInputRef.current.select()
    }
  }, [renamingId])

  const active: CustomProfile | undefined = profiles.find(p => p.id === activeId)

  // ── Mutators ────────────────────────────────────────────────────────────────

  const createProfile = () => {
    // crypto.randomUUID() is available in all modern WebViews used by Tauri.
    const id = crypto.randomUUID()
    const baseName = `Profile ${profiles.length + 1}`
    const newProfile: CustomProfile = { id, name: baseName, prompt: '' }
    onUpdate({
      customProfiles: [...profiles, newProfile],
      activeCustomProfileId: id,
    })
    // Open rename mode for the brand-new profile so the user can name it
    setRenamingId(id)
  }

  const setActive = (id: string) => {
    onUpdate({ activeCustomProfileId: id })
  }

  const updateActivePrompt = (prompt: string) => {
    if (!active) return
    onUpdate({
      customProfiles: profiles.map(p =>
        p.id === active.id ? { ...p, prompt: prompt.slice(0, MAX_PROMPT_CHARS) } : p
      ),
    })
  }

  const renameProfile = (id: string, newName: string) => {
    const trimmed = newName.trim().slice(0, MAX_NAME_CHARS)
    if (!trimmed) return
    onUpdate({
      customProfiles: profiles.map(p =>
        p.id === id ? { ...p, name: trimmed } : p
      ),
    })
  }

  const deleteProfile = (id: string) => {
    const remaining = profiles.filter(p => p.id !== id)
    // If the deleted profile was active, fall back to the first remaining one (or null).
    const nextActive = activeId === id
      ? (remaining[0]?.id ?? null)
      : activeId
    onUpdate({
      customProfiles: remaining,
      activeCustomProfileId: nextActive,
    })
  }

  // ── Empty state ─────────────────────────────────────────────────────────────

  if (profiles.length === 0) {
    return (
      <div style={{
        padding: 'var(--sp-4)',
        border: '1px dashed var(--border-subtle)',
        borderRadius: 'var(--radius-md)',
        background: 'var(--bg-secondary)',
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--sp-3)',
        alignItems: 'flex-start',
      }}>
        <div style={{ fontSize: 'var(--text-sm)', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
          You don't have any custom profiles yet. A profile is a saved system
          prompt — create one for each style of output you want (e.g. "Email
          tone", "Tweet under 280", "Bullet points only").
        </div>
        <Button variant="secondary" size="sm" onClick={createProfile}>
          <Plus size={12} style={{ marginRight: 4 }} /> Create your first profile
        </Button>
      </div>
    )
  }

  // ── Profile picker + editor ─────────────────────────────────────────────────

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
      {/* Profile chips: one per profile, click to activate */}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
        {profiles.map(profile => {
          const isActive = profile.id === activeId
          const isRenaming = profile.id === renamingId
          return (
            <div
              key={profile.id}
              style={{
                display: 'flex', alignItems: 'center', gap: 4,
                padding: '4px 8px',
                borderRadius: 'var(--radius)',
                border: `1px solid ${isActive ? 'var(--border-focus)' : 'var(--border-subtle)'}`,
                background: isActive ? 'var(--bg-active)' : 'var(--bg-secondary)',
                fontSize: 'var(--text-sm)',
              }}
            >
              {isRenaming ? (
                <input
                  ref={renameInputRef}
                  defaultValue={profile.name}
                  maxLength={MAX_NAME_CHARS}
                  onBlur={e => {
                    renameProfile(profile.id, e.target.value)
                    setRenamingId(null)
                  }}
                  onKeyDown={e => {
                    if (e.key === 'Enter') {
                      renameProfile(profile.id, e.currentTarget.value)
                      setRenamingId(null)
                    } else if (e.key === 'Escape') {
                      setRenamingId(null)
                    }
                  }}
                  style={{
                    background: 'var(--bg-primary)',
                    color: 'var(--text-primary)',
                    border: '1px solid var(--border-focus)',
                    borderRadius: 'var(--radius)',
                    padding: '2px 6px',
                    fontSize: 'var(--text-sm)',
                    fontFamily: 'inherit',
                    width: 140,
                  }}
                />
              ) : (
                <>
                  <button
                    onClick={() => setActive(profile.id)}
                    style={{
                      background: 'none', border: 'none', cursor: 'pointer',
                      color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
                      padding: '2px 4px',
                      fontFamily: 'inherit',
                      fontSize: 'var(--text-sm)',
                      fontWeight: isActive ? 500 : 400,
                    }}
                  >
                    {profile.name}
                  </button>
                  <button
                    onClick={() => setRenamingId(profile.id)}
                    title="Rename profile"
                    style={{
                      background: 'none', border: 'none', cursor: 'pointer',
                      color: 'var(--text-tertiary)',
                      padding: 2, display: 'flex', alignItems: 'center',
                    }}
                  >
                    <Pencil size={11} />
                  </button>
                  <button
                    onClick={() => deleteProfile(profile.id)}
                    title={`Delete "${profile.name}"`}
                    style={{
                      background: 'none', border: 'none', cursor: 'pointer',
                      color: '#FF6B6B',
                      padding: 2, display: 'flex', alignItems: 'center',
                    }}
                  >
                    <Trash2 size={11} />
                  </button>
                </>
              )}
            </div>
          )
        })}

        <button
          onClick={createProfile}
          title="Create a new profile"
          style={{
            display: 'flex', alignItems: 'center', gap: 4,
            padding: '4px 8px',
            borderRadius: 'var(--radius)',
            border: '1px dashed var(--border-default)',
            background: 'transparent',
            color: 'var(--text-secondary)',
            cursor: 'pointer',
            fontSize: 'var(--text-sm)',
            fontFamily: 'inherit',
          }}
        >
          <Plus size={12} /> New profile
        </button>
      </div>

      {/* Active profile's prompt editor */}
      {active ? (
        <div>
          <div style={{
            fontSize: 'var(--text-xs)',
            color: 'var(--text-secondary)',
            marginBottom: 6,
          }}>
            Editing <strong style={{ color: 'var(--text-primary)' }}>{active.name}</strong>.
            Hard rules (preserve language, no code fences, no commentary) are
            appended automatically — focus your prompt on style and goal.
          </div>
          <textarea
            value={active.prompt}
            maxLength={MAX_PROMPT_CHARS}
            onChange={e => updateActivePrompt(e.target.value)}
            placeholder="e.g. Rewrite this as a tweet under 280 chars in my casual tone."
            style={{
              width: '100%',
              minHeight: 140,
              background: 'var(--bg-secondary)',
              color: 'var(--text-primary)',
              border: '1px solid var(--border-default)',
              borderRadius: 'var(--radius)',
              padding: '8px 10px',
              fontSize: 'var(--text-sm)',
              fontFamily: 'inherit',
              resize: 'vertical',
            }}
          />
          <div style={{
            display: 'flex',
            justifyContent: 'space-between',
            fontSize: 'var(--text-xs)',
            color: 'var(--text-tertiary)',
            marginTop: 4,
          }}>
            <span>
              {active.prompt.trim() === ''
                ? 'Empty prompt → output falls back to Raw.'
                : ' '}
            </span>
            <span>{active.prompt.length} / {MAX_PROMPT_CHARS}</span>
          </div>
        </div>
      ) : (
        <div style={{
          padding: 'var(--sp-3)',
          fontSize: 'var(--text-sm)',
          color: 'var(--text-tertiary)',
          background: 'var(--bg-secondary)',
          borderRadius: 'var(--radius)',
        }}>
          Pick a profile to edit, or create a new one.
        </div>
      )}
    </div>
  )
}

// ---------- Professional Profiles ----------

const LOCALE_FLAGS: Record<AppLocale, string> = {
  en: '🇬🇧',
  cs: '🇨🇿',
  sk: '🇸🇰',
  pl: '🇵🇱',
}

const LOCALE_LABELS: Record<AppLocale, string> = {
  en: 'English',
  cs: 'Čeština',
  sk: 'Slovenčina',
  pl: 'Polski',
}

const OCCUPATION_LABELS: Record<Occupation, string> = {
  medicine:        'Medicine',
  law:             'Law',
  accounting:      'Accounting',
  marketing:       'Marketing',
  software:        'Software',
  generalBusiness: 'General',
  custom:          'Custom',
}

function ProfessionalProfiles({ settings, onUpdate }: Props) {
  const [profiles, setProfiles] = useState<OccupationProfile[]>([])
  const [loading, setLoading]   = useState(true)
  const [filterLocale, setFilterLocale] = useState<AppLocale | 'all'>(
    // Default the filter to the user's UI locale so a Czech user lands on Czech profiles.
    settings.appLocale === 'en' ? 'all' : settings.appLocale,
  )
  const [expandedId, setExpandedId] = useState<string | null>(null)

  // Load + reload when user clones change (the Rust side emits settings-changed
  // after every save, and useSettings reloads the Settings object — but the
  // profile list is its own resource, so we refetch when our own clones array
  // changes).
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    api.listOccupationProfiles()
      .then(list => { if (!cancelled) setProfiles(list) })
      .catch(() => { /* surfaced via Toast in useSettings */ })
      .finally(() => { if (!cancelled) setLoading(false) })
    return () => { cancelled = true }
  }, [settings.userOccupationProfiles])

  const visibleProfiles = useMemo(
    () => filterLocale === 'all'
      ? profiles
      : profiles.filter(p => p.locale === filterLocale),
    [profiles, filterLocale],
  )

  const activeId = settings.activeOccupationProfileId

  async function activate(id: string | null) {
    await api.setActiveOccupationProfile(id)
    onUpdate({ activeOccupationProfileId: id })
  }

  if (loading) {
    return <div style={{ color: 'var(--text-tertiary)', fontSize: 'var(--text-sm)' }}>Loading profiles…</div>
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
      <p style={{
        color: 'var(--text-secondary)',
        fontSize: 'var(--text-sm)',
        margin: 0,
        lineHeight: 1.5,
      }}>
        Pre-built profiles for specific occupations and languages. They tell the LLM how
        to clean your transcript and which English technical terms must stay in English
        even when you speak Czech, Slovak, or Polish. Activating a profile overrides
        Output Mode below.
      </p>

      {/* Locale filter chips */}
      <div style={{ display: 'flex', gap: 'var(--sp-2)', flexWrap: 'wrap' }}>
        <FilterChip
          active={filterLocale === 'all'}
          onClick={() => setFilterLocale('all')}
          label="All"
        />
        {(['cs', 'sk', 'pl'] as AppLocale[]).map(loc => (
          <FilterChip
            key={loc}
            active={filterLocale === loc}
            onClick={() => setFilterLocale(loc)}
            label={`${LOCALE_FLAGS[loc]}  ${LOCALE_LABELS[loc]}`}
          />
        ))}
      </div>

      {/* Active-profile banner */}
      {activeId && (
        <div style={{
          padding: 'var(--sp-3)',
          background: 'var(--bg-active, rgba(107,139,255,0.1))',
          border: '1px solid var(--border-focus)',
          borderRadius: 'var(--radius)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 'var(--sp-3)',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-2)' }}>
            <Check size={16} />
            <span style={{ fontSize: 'var(--text-sm)' }}>
              Active: <strong>{profiles.find(p => p.id === activeId)?.name ?? '(missing profile)'}</strong>
            </span>
          </div>
          <Button onClick={() => activate(null)}>Deactivate</Button>
        </div>
      )}

      {/* Profile grid */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))',
        gap: 'var(--sp-3)',
      }}>
        {visibleProfiles.map(p => (
          <ProfileCard
            key={p.id}
            profile={p}
            active={p.id === activeId}
            expanded={p.id === expandedId}
            onActivate={() => activate(p.id)}
            onToggleExpand={() => setExpandedId(expandedId === p.id ? null : p.id)}
          />
        ))}
        {visibleProfiles.length === 0 && (
          <div style={{ color: 'var(--text-tertiary)', fontSize: 'var(--text-sm)' }}>
            No profiles for this language.
          </div>
        )}
      </div>
    </div>
  )
}

function FilterChip({ active, onClick, label }: { active: boolean; onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: 'var(--sp-1) var(--sp-3)',
        background: active ? 'var(--bg-active, rgba(107,139,255,0.15))' : 'var(--bg-secondary)',
        border: `1px solid ${active ? 'var(--border-focus)' : 'var(--border)'}`,
        color: active ? 'var(--text-primary)' : 'var(--text-secondary)',
        borderRadius: '999px',
        fontSize: 'var(--text-sm)',
        cursor: 'pointer',
      }}
    >
      {label}
    </button>
  )
}

function ProfileCard({
  profile, active, expanded, onActivate, onToggleExpand,
}: {
  profile: OccupationProfile
  active: boolean
  expanded: boolean
  onActivate: () => void
  onToggleExpand: () => void
}) {
  return (
    <div style={{
      padding: 'var(--sp-3)',
      border: `1px solid ${active ? 'var(--border-focus)' : 'var(--border)'}`,
      borderRadius: 'var(--radius)',
      background: active ? 'var(--bg-active, rgba(107,139,255,0.05))' : 'var(--bg-secondary)',
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--sp-2)',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-2)' }}>
        <span style={{ fontSize: 16 }}>{LOCALE_FLAGS[profile.locale]}</span>
        <strong style={{ fontSize: 'var(--text-sm)' }}>{profile.name}</strong>
      </div>
      <div style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        fontSize: 'var(--text-xs, 11px)',
        color: 'var(--text-tertiary)',
      }}>
        <Briefcase size={11} />
        {OCCUPATION_LABELS[profile.occupation]}
      </div>
      <p style={{
        margin: 0,
        fontSize: 'var(--text-xs, 12px)',
        color: 'var(--text-secondary)',
        lineHeight: 1.4,
        minHeight: 32,
      }}>
        {profile.description || ' '}
      </p>
      {profile.preserveTerms.length > 0 && (
        <div style={{
          fontSize: 'var(--text-xs, 11px)',
          color: 'var(--text-tertiary)',
        }}>
          Keeps in English: <span style={{ color: 'var(--text-secondary)' }}>
            {profile.preserveTerms.slice(0, 5).join(', ')}
            {profile.preserveTerms.length > 5 && ` +${profile.preserveTerms.length - 5}`}
          </span>
        </div>
      )}
      <div style={{ display: 'flex', gap: 'var(--sp-2)', marginTop: 'auto' }}>
        <Button onClick={onActivate}>
          {active ? 'Active' : 'Activate'}
        </Button>
        <Button onClick={onToggleExpand}>
          {expanded ? 'Hide prompt' : 'Show prompt'}
        </Button>
      </div>
      {expanded && (
        <pre style={{
          margin: 0,
          padding: 'var(--sp-2)',
          background: 'var(--bg-tertiary, #0a0a0a)',
          borderRadius: 4,
          fontSize: 'var(--text-xs, 11px)',
          color: 'var(--text-tertiary)',
          whiteSpace: 'pre-wrap',
          maxHeight: 240,
          overflowY: 'auto',
        }}>
          {profile.prompt}
        </pre>
      )}
    </div>
  )
}
