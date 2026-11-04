import { useEffect } from 'react'

type ToastProps = {
  message: string | null
  onDismiss: () => void
  /** Auto-dismiss delay in ms (default 4000) */
  ttl?: number
  kind?: 'error' | 'info'
}

export function Toast({ message, onDismiss, ttl = 4000, kind = 'error' }: ToastProps) {
  useEffect(() => {
    if (!message) return
    const t = window.setTimeout(onDismiss, ttl)
    return () => window.clearTimeout(t)
  }, [message, ttl, onDismiss])

  useEffect(() => {
    if (!message) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onDismiss()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [message, onDismiss])

  if (!message) return null

  const isError = kind === 'error'
  const bg = isError ? 'rgba(220, 38, 38, 0.15)' : 'rgba(59, 130, 246, 0.15)'
  const border = isError ? 'rgba(220, 38, 38, 0.45)' : 'rgba(59, 130, 246, 0.45)'
  const fg = isError ? '#FCA5A5' : '#93C5FD'

  return (
    <div
      role={isError ? 'alert' : 'status'}
      aria-live={isError ? 'assertive' : 'polite'}
      style={{
        position: 'fixed',
        top: 12,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
        background: bg,
        border: `1px solid ${border}`,
        color: fg,
        padding: '8px 14px',
        borderRadius: 'var(--radius)',
        fontSize: 'var(--text-sm)',
        maxWidth: 'calc(100% - 32px)',
        boxShadow: '0 4px 12px rgba(0,0,0,0.35)',
        display: 'flex',
        alignItems: 'center',
        gap: 10,
      }}
    >
      <span>{message}</span>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        style={{
          background: 'transparent',
          border: 'none',
          color: fg,
          cursor: 'pointer',
          fontSize: 14,
          lineHeight: 1,
          padding: '2px 4px',
          marginLeft: 'auto',
        }}
      >
        ×
      </button>
    </div>
  )
}
