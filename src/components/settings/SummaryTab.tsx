import { Toggle } from '../ui/Toggle'
import { Section } from '../ui/Section'
import { RadioCard, RadioCardGroup } from '../ui/RadioCard'
import type { Settings, SummaryType } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

const SUMMARY_TYPES: { value: SummaryType; label: string; desc: string }[] = [
  { value: 'keyPoints', label: 'Key Points', desc: 'Bullet-point summary of main ideas' },
  { value: 'actionItems', label: 'Action Items', desc: 'Extracted tasks and next steps' },
  { value: 'tldr', label: 'TL;DR', desc: 'One-paragraph condensed version' },
]

export function SummaryTab({ settings, onUpdate }: Props) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>
      <Section title="Summary">
        <Toggle
          label="Enable summary mode"
          description="Generate a concise summary instead of the full transcript"
          checked={settings.summaryEnabled}
          onChange={val => onUpdate({ summaryEnabled: val })}
        />
      </Section>

      {settings.summaryEnabled && (
        <Section title="Summary Type">
          <RadioCardGroup label="Summary Type">
            {SUMMARY_TYPES.map(t => (
              <RadioCard
                key={t.value}
                label={t.label}
                description={t.desc}
                checked={settings.summaryType === t.value}
                onChange={() => onUpdate({ summaryType: t.value })}
              />
            ))}
          </RadioCardGroup>
        </Section>
      )}
    </div>
  )
}
