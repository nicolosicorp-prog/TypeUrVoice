import { Toggle } from '../ui/Toggle'
import { Select } from '../ui/Select'
import { Section } from '../ui/Section'
import { LANGUAGES } from '../../lib/constants'
import type { Settings } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

export function TranslationTab({ settings, onUpdate }: Props) {
  const sourceLangs = LANGUAGES
  const targetLangs = LANGUAGES.filter(l => l.code !== 'auto')

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>
      <Section title="Speech Recognition">
        <Select
          label="Source language"
          value={settings.sourceLanguage}
          onChange={e => onUpdate({ sourceLanguage: e.target.value })}
          options={sourceLangs.map(l => ({ value: l.code, label: l.label }))}
          description="Language you speak in. Auto-detect works for most languages but setting it explicitly is more accurate."
        />
      </Section>

      <Section title="Translation">
        <Toggle
          label="Enable translation"
          description="Output your speech in a different language than you spoke"
          checked={settings.translationEnabled}
          onChange={val => onUpdate({ translationEnabled: val })}
        />

        {settings.translationEnabled && (
          <Select
            label="Target language"
            value={settings.targetLanguage}
            onChange={e => onUpdate({ targetLanguage: e.target.value })}
            options={targetLangs.map(l => ({ value: l.code, label: l.label }))}
            description="Language the output will be written in."
          />
        )}
      </Section>
    </div>
  )
}

