import { InputHTMLAttributes, useState } from 'react'
import { Eye, EyeOff } from 'lucide-react'

interface Props extends InputHTMLAttributes<HTMLInputElement> {
  label?: string
  description?: string
  error?: string
  suffix?: React.ReactNode
}

export function Input({ label, description, error, suffix, type, style, ...props }: Props) {
  const [showPwd, setShowPwd] = useState(false)
  const isPassword = type === 'password'

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      {label && (
        <label style={{ fontSize: 'var(--text-sm)', color: 'var(--text-primary)', fontWeight: 500 }}>
          {label}
        </label>
      )}
      <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
        <input
          {...props}
          type={isPassword && showPwd ? 'text' : type}
          style={{
            width: '100%',
            height: 30,
            background: 'var(--bg-secondary)',
            border: `1px solid ${error ? '#442222' : 'var(--border-default)'}`,
            borderRadius: 'var(--radius)',
            color: 'var(--text-primary)',
            fontSize: 'var(--text-sm)',
            fontFamily: 'inherit',
            padding: `0 ${isPassword || suffix ? '32px' : '10px'} 0 10px`,
            outline: 'none',
            transition: 'border-color var(--t-normal)',
            ...style,
          }}
          onFocus={e => { e.target.style.borderColor = 'var(--border-focus)' }}
          onBlur={e => { e.target.style.borderColor = error ? '#442222' : 'var(--border-default)' }}
        />
        {isPassword && (
          <button
            type="button"
            onClick={() => setShowPwd(v => !v)}
            style={{
              position: 'absolute', right: 8, background: 'none', border: 'none',
              color: 'var(--text-tertiary)', cursor: 'pointer', padding: 0, display: 'flex',
            }}
          >
            {showPwd ? <EyeOff size={14} /> : <Eye size={14} />}
          </button>
        )}
        {suffix && !isPassword && (
          <div style={{ position: 'absolute', right: 8, color: 'var(--text-tertiary)', display: 'flex' }}>
            {suffix}
          </div>
        )}
      </div>
      {description && !error && (
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)' }}>{description}</div>
      )}
      {error && (
        <div style={{ fontSize: 'var(--text-xs)', color: '#FF6B6B' }}>{error}</div>
      )}
    </div>
  )
}
