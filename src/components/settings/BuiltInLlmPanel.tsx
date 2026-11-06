import { useState, useEffect, useCallback } from 'react'
import { Download, Trash2, Check } from 'lucide-react'
import { listen } from '@tauri-apps/api/event'
import { Button } from '../ui/Button'
import { Toggle } from '../ui/Toggle'
import { api } from '../../lib/tauri'
import type { Settings, BuiltinLlmModel, BuiltinLlmStatus } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

/** Text Processing → "Built-in (on this Mac)": model list, auto load/unload, status. */
export function BuiltInLlmPanel({ settings, onUpdate }: Props) {
  const [models, setModels] = useState<BuiltinLlmModel[]>([])
  const [status, setStatus] = useState<BuiltinLlmStatus>({ state: 'unloaded' })
  const [progress, setProgress] = useState<Record<string, number>>({})
  const [downloading, setDownloading] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(() => {
    api.listBuiltinLlmModels().then(setModels).catch(() => {})
  }, [])

  useEffect(() => {
    refresh()
    api.getBuiltinLlmStatus().then(setStatus).catch(() => {})
    const unlisten = [
      listen<BuiltinLlmStatus>('builtin-llm-status', e => setStatus(e.payload)),
      listen<{ model: string; progress: number }>('llm-download-progress', e =>
        setProgress(p => ({ ...p, [e.payload.model]: e.payload.progress }))),
      listen<string>('llm-download-done', () => { setDownloading(null); refresh() }),
      listen<{ model: string; error: string }>('llm-download-error', e => {
        setDownloading(null)
        setError(e.payload.error)
      }),
    ]
    return () => { unlisten.forEach(p => p.then(fn => fn())) }
  }, [refresh])

  const download = async (key: string) => {
    setError(null)
    setDownloading(key)
    setProgress(p => ({ ...p, [key]: 0 }))
    try {
      await api.downloadBuiltinLlmModel(key)
    } catch (e) {
      setDownloading(null)
      setError(String(e))
    }
  }

  const remove = async (key: string) => {
    await api.deleteBuiltinLlmModel(key)
    refresh()
  }

  const selected = models.find(m => m.key === settings.builtinLlmModel)
  const statusLabel = (() => {
    switch (status.state) {
      case 'ready': return { color: 'var(--accent-green, #34D399)', text: `Loaded — ${modelName(models, status.model)}` }
      case 'loading': return { color: 'var(--accent-yellow, #FBBF24)', text: `Loading ${modelName(models, status.model)}…` }
      case 'error': return { color: 'var(--accent-red, #FF6B6B)', text: status.message }
      default: return { color: 'var(--text-tertiary)', text: 'Not loaded' }
    }
  })()

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)', marginTop: 12 }}>
      <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
        Runs a small AI model directly inside TypeUrVoice on your Mac's GPU. No Ollama or LM Studio,
        no API key, and your text never leaves this computer.
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }} role="radiogroup" aria-label="Built-in model">
        {models.map(m => {
          const active = m.key === settings.builtinLlmModel
          const isDownloading = downloading === m.key
          return (
            <div
              key={m.key}
              role="radio"
              aria-checked={active}
              tabIndex={m.downloaded ? 0 : -1}
              onClick={() => m.downloaded && onUpdate({ builtinLlmModel: m.key })}
              onKeyDown={e => {
                if (m.downloaded && (e.key === ' ' || e.key === 'Enter')) {
                  e.preventDefault()
                  onUpdate({ builtinLlmModel: m.key })
                }
              }}
              style={{
                display: 'flex', alignItems: 'center', gap: 10,
                padding: '8px 10px', borderRadius: 'var(--radius)',
                border: `1px solid ${active && m.downloaded ? 'var(--border-focus)' : 'var(--border-subtle)'}`,
                background: active && m.downloaded ? 'var(--bg-active)' : 'var(--bg-secondary)',
                cursor: m.downloaded ? 'pointer' : 'default',
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
                  <span style={{ fontSize: 'var(--text-sm)', fontWeight: active ? 500 : 400 }}>{m.name}</span>
                  {m.recommended && (
                    <span style={{
                      fontSize: 10, padding: '1px 5px', borderRadius: 999,
                      background: 'rgba(107, 139, 255, 0.22)', color: 'var(--border-focus)', fontWeight: 600,
                    }}>RECOMMENDED</span>
                  )}
                  <span style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>
                    {formatSize(m.sizeMb)} · ~{m.ramGb} GB RAM
                  </span>
                </div>
                <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 2 }}>{m.note}</div>
                {isDownloading && (
                  <div style={{ marginTop: 4 }}>
                    <div style={{ height: 2, background: 'var(--border-subtle)', borderRadius: 1, overflow: 'hidden' }}>
                      <div style={{
                        height: '100%', background: 'var(--accent-blue)', borderRadius: 1,
                        width: `${Math.round((progress[m.key] ?? 0) * 100)}%`,
                        transition: 'width 300ms ease-out',
                      }} />
                    </div>
                    <div style={{ fontSize: 10, color: 'var(--text-tertiary)', marginTop: 2 }}>
                      {Math.round((progress[m.key] ?? 0) * 100)}%
                    </div>
                  </div>
                )}
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }} onClick={e => e.stopPropagation()}>
                {m.downloaded ? (
                  <>
                    {active && <Check size={14} style={{ color: 'var(--border-focus)' }} aria-hidden />}
                    <Button onClick={() => remove(m.key)} aria-label={`Delete ${m.name}`}>
                      <Trash2 size={12} />
                    </Button>
                  </>
                ) : (
                  <Button onClick={() => download(m.key)} disabled={downloading !== null} aria-label={`Download ${m.name}`}>
                    <Download size={12} />
                  </Button>
                )}
              </div>
            </div>
          )
        })}
      </div>

      {selected && !selected.downloaded && (
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--accent-yellow, #FBBF24)' }}>
          Download {selected.name} (or pick a downloaded model) to use Built-in processing.
        </div>
      )}
      {error && (
        <div role="alert" style={{ fontSize: 'var(--text-xs)', color: 'var(--accent-red, #FF6B6B)', whiteSpace: 'pre-wrap' }}>
          {error}
        </div>
      )}

      <Toggle
        label="Load automatically and unload when idle"
        description={settings.builtinLlmAuto
          ? 'The model loads when you start recording and frees its memory after you stop using it.'
          : 'The model stays loaded from launch until you quit TypeUrVoice — fastest, but always uses memory.'}
        checked={settings.builtinLlmAuto}
        onChange={val => onUpdate({ builtinLlmAuto: val })}
      />
      {settings.builtinLlmAuto && (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 'var(--text-xs)', color: 'var(--text-secondary)' }}>
          Unload after
          <select
            value={settings.builtinLlmIdleMinutes}
            onChange={e => onUpdate({ builtinLlmIdleMinutes: Number(e.target.value) })}
            style={{
              height: 26, background: 'var(--bg-secondary)', color: 'var(--text-primary)',
              border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
              padding: '0 6px', fontSize: 'var(--text-xs)', fontFamily: 'inherit',
            }}
          >
            {[1, 2, 5, 10, 15, 30, 60].map(n => (
              <option key={n} value={n}>{n} min</option>
            ))}
          </select>
          of inactivity
        </label>
      )}

      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span aria-hidden style={{ width: 8, height: 8, borderRadius: '50%', background: statusLabel.color, flexShrink: 0 }} />
        <span role="status" style={{
          fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', flex: 1, minWidth: 0,
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }} title={statusLabel.text}>
          {statusLabel.text}
        </span>
        {(status.state === 'ready' || status.state === 'loading') && (
          <Button onClick={() => api.unloadBuiltinLlm()}>Unload now</Button>
        )}
      </div>
    </div>
  )
}

function modelName(models: BuiltinLlmModel[], key: string) {
  return models.find(m => m.key === key)?.name ?? key
}

function formatSize(mb: number) {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`
}
