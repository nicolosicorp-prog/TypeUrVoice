import { useState, useEffect, useCallback } from 'react'
import { Copy, Trash2, Clock } from 'lucide-react'
import { Button } from '../ui/Button'
import { Select } from '../ui/Select'
import { api } from '../../lib/tauri'
import type { Settings, HistoryEntry } from '../../types'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

const RETENTION_OPTIONS = [
  { value: '7', label: '7 days' },
  { value: '30', label: '30 days' },
  { value: '90', label: '90 days' },
  { value: '365', label: '1 year' },
  { value: '99999', label: 'Forever' },
]

export function HistoryTab({ settings, onUpdate }: Props) {
  const [entries, setEntries] = useState<HistoryEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [expanded, setExpanded] = useState<number | null>(null)
  const [confirmClear, setConfirmClear] = useState(false)

  const loadHistory = () => {
    setLoading(true)
    api.getHistory(50, 0)
      .then(setEntries)
      .catch(console.error)
      .finally(() => setLoading(false))
  }

  useEffect(() => { loadHistory() }, [])

  const deleteEntry = async (id: number) => {
    await api.deleteHistoryEntry(id)
    setEntries(e => e.filter(x => x.id !== id))
  }

  const copyEntry = async (text: string) => {
    try {
      await writeText(text)
    } catch {
      // fallback
    }
  }

  const clearAll = async () => {
    if (!confirmClear) {
      setConfirmClear(true)
      setTimeout(() => setConfirmClear(false), 3000)
      return
    }
    await api.clearHistory()
    setEntries([])
    setConfirmClear(false)
  }

  // Escape cancels the pending-confirm state so the two-click pattern is
  // obvious and reversible (webview confirm() dialogs don't work here).
  const onKey = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape' && confirmClear) setConfirmClear(false)
  }, [confirmClear])
  useEffect(() => {
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onKey])

  const formatDate = (dt: string) => {
    const d = new Date(dt)
    const now = new Date()
    const diff = (now.getTime() - d.getTime()) / 1000
    if (diff < 60) return 'just now'
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
    return d.toLocaleDateString()
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-4)', height: '100%' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600 }}>Recent Activity</div>
        <div style={{ display: 'flex', gap: 'var(--sp-2)', alignItems: 'center' }}>
          <Select
            value={String(settings.historyRetentionDays)}
            onChange={e => onUpdate({ historyRetentionDays: Number(e.target.value) })}
            options={RETENTION_OPTIONS}
            style={{ height: 26, fontSize: 'var(--text-xs)' }}
          />
          {entries.length > 0 && (
            <>
              {confirmClear && (
                <span
                  aria-live="polite"
                  style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)' }}
                >
                  Click again within 3s · Esc to cancel
                </span>
              )}
              <Button variant="ghost" size="sm" onClick={clearAll}
                style={confirmClear ? { color: '#FF6B6B' } : undefined}>
                {confirmClear ? 'Confirm Clear?' : 'Clear'}
              </Button>
            </>
          )}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 4 }}>
        {loading && (
          <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', fontSize: 'var(--text-sm)', padding: 24 }}>Loading...</div>
        )}

        {!loading && entries.length === 0 && (
          <div style={{
            padding: '32px 16px', borderRadius: 'var(--radius-md)',
            border: '1px dashed var(--border-subtle)', textAlign: 'center',
          }}>
            <Clock size={20} style={{ color: 'var(--text-tertiary)', marginBottom: 8 }} />
            <div style={{ fontSize: 'var(--text-sm)', color: 'var(--text-tertiary)' }}>No transcriptions yet.</div>
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 4 }}>Press your shortcut to start recording.</div>
          </div>
        )}

        {entries.map(entry => (
          <div
            key={entry.id}
            style={{
              borderRadius: 'var(--radius)',
              border: '1px solid var(--border-subtle)',
              background: expanded === entry.id ? 'var(--bg-tertiary)' : 'var(--bg-secondary)',
              overflow: 'hidden',
              transition: 'background var(--t-normal)',
            }}
          >
            <div
              onClick={() => setExpanded(expanded === entry.id ? null : entry.id)}
              style={{
                display: 'flex', alignItems: 'center', gap: 'var(--sp-2)',
                padding: '7px 10px', cursor: 'pointer',
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{
                  fontSize: 'var(--text-xs)', color: 'var(--text-primary)',
                  overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                }}>
                  {entry.processedText.slice(0, 80)}{entry.processedText.length > 80 ? '…' : ''}
                </div>
                <div style={{ display: 'flex', gap: 6, marginTop: 2, alignItems: 'center' }}>
                  <span style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{formatDate(entry.createdAt)}</span>
                  <span style={{
                    fontSize: 9, padding: '1px 5px', borderRadius: 999,
                    background: 'var(--bg-hover)', color: 'var(--text-tertiary)',
                    textTransform: 'uppercase', letterSpacing: '0.04em',
                  }}>{entry.mode}</span>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 4, flexShrink: 0 }}>
                <button
                  onClick={e => { e.stopPropagation(); copyEntry(entry.processedText) }}
                  style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', padding: 4, display: 'flex' }}
                >
                  <Copy size={12} />
                </button>
                <button
                  onClick={e => { e.stopPropagation(); deleteEntry(entry.id) }}
                  style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', padding: 4, display: 'flex' }}
                >
                  <Trash2 size={12} />
                </button>
              </div>
            </div>

            {expanded === entry.id && (
              <div style={{
                padding: '0 10px 10px',
                borderTop: '1px solid var(--border-subtle)',
                fontSize: 'var(--text-xs)', color: 'var(--text-secondary)',
                lineHeight: 1.6, whiteSpace: 'pre-wrap',
                maxHeight: 120, overflowY: 'auto',
              }}>
                {entry.processedText}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
