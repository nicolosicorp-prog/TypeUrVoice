import { CSSProperties } from 'react'

interface RadioCardProps {
  label: string
  description?: string
  checked: boolean
  onChange: () => void
  /** Visual variant: 'dot' matches OutputTab/SummaryTab, 'native' matches GeneralTab */
  variant?: 'dot' | 'native'
}

export function RadioCard({ label, description, checked, onChange, variant = 'dot' }: RadioCardProps) {
  const base: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--sp-3)',
    padding: '8px 10px',
    borderRadius: 'var(--radius)',
    border: `1px solid ${checked ? 'var(--border-focus)' : 'var(--border-subtle)'}`,
    background: checked ? 'var(--bg-active)' : 'transparent',
    color: 'var(--text-primary)',
    cursor: 'pointer',
    textAlign: 'left',
    transition: 'all var(--t-normal)',
    fontFamily: 'inherit',
    width: '100%',
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault()
      onChange()
    }
  }

  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      onClick={onChange}
      onKeyDown={onKeyDown}
      style={base}
    >
      {variant === 'dot' ? (
        <span
          aria-hidden
          style={{
            width: 14,
            height: 14,
            borderRadius: '50%',
            flexShrink: 0,
            border: `2px solid ${checked ? 'var(--text-primary)' : 'var(--border-default)'}`,
            background: checked ? 'var(--text-primary)' : 'transparent',
            transition: 'all var(--t-normal)',
          }}
        />
      ) : (
        <span
          aria-hidden
          style={{
            width: 14,
            height: 14,
            borderRadius: '50%',
            flexShrink: 0,
            border: `2px solid ${checked ? 'var(--text-primary)' : 'var(--border-default)'}`,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          {checked && (
            <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--text-primary)' }} />
          )}
        </span>
      )}
      <span style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <span style={{ fontSize: 'var(--text-sm)', fontWeight: checked ? 500 : 400 }}>{label}</span>
        {description && (
          <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', marginTop: 2 }}>
            {description}
          </span>
        )}
      </span>
    </button>
  )
}

/** Wrapper group that gives a semantic radiogroup role to a set of RadioCards. */
export function RadioCardGroup({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div role="radiogroup" aria-label={label} style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)' }}>
      {children}
    </div>
  )
}
