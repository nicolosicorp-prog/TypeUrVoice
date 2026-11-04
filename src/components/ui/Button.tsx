import { ButtonHTMLAttributes, CSSProperties, ReactNode } from 'react'

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md' | 'lg'
  loading?: boolean
  children: ReactNode
}

const base: CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 6,
  border: 'none',
  cursor: 'pointer',
  fontFamily: 'inherit',
  fontWeight: 500,
  borderRadius: 'var(--radius)',
  transition: 'background var(--t-normal), color var(--t-normal), opacity var(--t-normal)',
  whiteSpace: 'nowrap',
  outline: 'none',
}

const variants: Record<NonNullable<Props['variant']>, CSSProperties> = {
  primary: { background: 'var(--text-primary)', color: 'var(--text-inverse)' },
  secondary: { background: 'var(--bg-tertiary)', color: 'var(--text-primary)', border: '1px solid var(--border-default)' },
  ghost: { background: 'transparent', color: 'var(--text-secondary)' },
  danger: { background: 'transparent', color: '#FF4444', border: '1px solid #441111' },
}

const sizes: Record<NonNullable<Props['size']>, CSSProperties> = {
  sm: { fontSize: 'var(--text-xs)', padding: '4px 10px', height: 26 },
  md: { fontSize: 'var(--text-sm)', padding: '5px 12px', height: 30 },
  lg: { fontSize: 'var(--text-base)', padding: '6px 16px', height: 34 },
}

export function Button({
  variant = 'secondary',
  size = 'md',
  loading = false,
  disabled,
  children,
  style,
  ...props
}: Props) {
  const isDisabled = disabled || loading
  return (
    <button
      {...props}
      disabled={isDisabled}
      style={{
        ...base,
        ...variants[variant],
        ...sizes[size],
        opacity: isDisabled ? 0.4 : 1,
        cursor: isDisabled ? 'not-allowed' : 'pointer',
        ...style,
      }}
      onMouseEnter={(e) => {
        if (!isDisabled) {
          const el = e.currentTarget
          if (variant === 'primary') el.style.opacity = '0.88'
          else el.style.background = 'var(--bg-hover)'
        }
        props.onMouseEnter?.(e)
      }}
      onMouseLeave={(e) => {
        if (!isDisabled) {
          const el = e.currentTarget
          if (variant === 'primary') el.style.opacity = '1'
          else if (variant === 'ghost') el.style.background = 'transparent'
          else el.style.background = variant === 'secondary' ? 'var(--bg-tertiary)' : 'transparent'
        }
        props.onMouseLeave?.(e)
      }}
    >
      {loading ? <Spinner /> : children}
    </button>
  )
}

function Spinner() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" className="animate-spin" style={{ display: 'block' }}>
      <circle cx="6" cy="6" r="4.5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeDasharray="14" strokeDashoffset="0" strokeLinecap="round" />
    </svg>
  )
}
