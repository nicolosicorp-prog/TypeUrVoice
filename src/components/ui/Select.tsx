import { SelectHTMLAttributes } from 'react'

interface Option { value: string; label: string }

interface Props extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string
  description?: string
  options: Option[]
}

export function Select({ label, description, options, style, ...props }: Props) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      {label && (
        <label style={{ fontSize: 'var(--text-sm)', color: 'var(--text-primary)', fontWeight: 500 }}>
          {label}
        </label>
      )}
      <select
        {...props}
        style={{
          height: 30,
          background: 'var(--bg-secondary)',
          border: '1px solid var(--border-default)',
          borderRadius: 'var(--radius)',
          color: 'var(--text-primary)',
          fontSize: 'var(--text-sm)',
          fontFamily: 'inherit',
          padding: '0 8px',
          outline: 'none',
          cursor: 'pointer',
          appearance: 'none',
          backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' fill='none'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%23888' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`,
          backgroundRepeat: 'no-repeat',
          backgroundPosition: 'right 8px center',
          paddingRight: 28,
          transition: 'border-color var(--t-normal)',
          ...style,
        }}
        onFocus={e => { e.target.style.borderColor = 'var(--border-focus)' }}
        onBlur={e => { e.target.style.borderColor = 'var(--border-default)' }}
      >
        {options.map(opt => (
          <option key={opt.value} value={opt.value} style={{ background: '#1A1A1A' }}>
            {opt.label}
          </option>
        ))}
      </select>
      {description && (
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)' }}>{description}</div>
      )}
    </div>
  )
}
