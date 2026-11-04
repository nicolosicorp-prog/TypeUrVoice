interface Props {
  checked: boolean
  onChange: (val: boolean) => void
  label?: string
  description?: string
  disabled?: boolean
}

export function Toggle({ checked, onChange, label, description, disabled }: Props) {
  return (
    <label style={{
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      gap: '12px', cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.4 : 1,
    }}>
      {(label || description) && (
        <div>
          {label && <div style={{ fontSize: 'var(--text-sm)', color: 'var(--text-primary)', fontWeight: 500 }}>{label}</div>}
          {description && <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', marginTop: 2 }}>{description}</div>}
        </div>
      )}
      <input
        type="checkbox"
        role="switch"
        checked={checked}
        disabled={disabled}
        onChange={e => !disabled && onChange(e.target.checked)}
        style={{
          position: 'absolute',
          width: 1,
          height: 1,
          padding: 0,
          margin: -1,
          overflow: 'hidden',
          clip: 'rect(0, 0, 0, 0)',
          whiteSpace: 'nowrap',
          border: 0,
        }}
      />
      <div
        style={{
          flexShrink: 0,
          width: 36,
          height: 20,
          borderRadius: 999,
          background: checked ? 'var(--text-primary)' : 'var(--bg-hover)',
          border: `1px solid ${checked ? 'transparent' : 'var(--border-default)'}`,
          position: 'relative',
          transition: 'background var(--t-normal)',
          cursor: disabled ? 'not-allowed' : 'pointer',
        }}
      >
        <div style={{
          position: 'absolute',
          top: 2,
          left: checked ? 18 : 2,
          width: 14,
          height: 14,
          borderRadius: '50%',
          background: checked ? 'var(--text-inverse)' : 'var(--text-tertiary)',
          transition: 'left var(--t-normal)',
        }} />
      </div>
    </label>
  )
}
