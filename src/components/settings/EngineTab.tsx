import { useState, useEffect, useCallback } from 'react'
import { CheckCircle, XCircle, Download, Check, HardDrive, Trash2 } from 'lucide-react'
import { Input } from '../ui/Input'
import { Button } from '../ui/Button'
import { Section } from '../ui/Section'
import { BuiltInLlmPanel } from './BuiltInLlmPanel'
import { api } from '../../lib/tauri'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  Settings,
  TextProcessor,
  LocalModel,
  OpenAiCompatPreset,
  ProviderInfo,
  CloudTranscriptionProvider,
} from '../../types'

/** Mirrors `transcription::cloud::transcription_models` — default first. */
const CLOUD_STT_MODELS: Record<CloudTranscriptionProvider, { id: string; label: string }[]> = {
  openai: [
    { id: 'gpt-transcribe', label: 'gpt-transcribe — most accurate (recommended)' },
    { id: 'gpt-4o-mini-transcribe', label: 'gpt-4o-mini-transcribe — cheaper' },
    { id: 'whisper-1', label: 'whisper-1 — legacy' },
  ],
  groq: [
    { id: 'whisper-large-v3-turbo', label: 'Whisper Large v3 Turbo — fastest (recommended)' },
    { id: 'whisper-large-v3', label: 'Whisper Large v3 — slightly more accurate' },
  ],
}

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

interface LocalModelInfo {
  key: string
  name: string
  downloaded: boolean
  sizeMb: number
}

// ── Provider catalogue (UI side — display labels only; canonical data lives in Rust) ──
const PROVIDERS: { value: TextProcessor; label: string; sub?: string }[] = [
  { value: 'builtin',      label: 'Built-in (on this Mac)', sub: 'Private, free, no setup' },
  { value: 'openai',       label: 'OpenAI',                 sub: 'GPT-6 Luna / 6.1 Terra / 6.1 Sol' },
  { value: 'anthropic',    label: 'Anthropic Claude',       sub: 'Claude Haiku/Sonnet/Opus 5.5, Fable 5.1' },
  { value: 'gemini',       label: 'Google Gemini',          sub: 'Gemini 3.5 Flash-Lite / 3.8 Flash / 4 Argon' },
  { value: 'openaicompat', label: 'OpenAI-compatible (cloud)', sub: 'Grok, Groq, DeepSeek, Mistral, OpenRouter…' },
  { value: 'ollama',       label: 'Ollama (local)',         sub: 'http://localhost:11434' },
  { value: 'lmstudio',     label: 'LM Studio (local)',      sub: 'http://localhost:1234' },
]

const COMPAT_PRESETS: { value: OpenAiCompatPreset; label: string; tooltip?: string }[] = [
  { value: 'grok',       label: 'xAI Grok' },
  { value: 'groq',       label: 'Groq', tooltip: 'Different from xAI Grok — Groq is a fast inference host.' },
  { value: 'deepseek',   label: 'DeepSeek' },
  { value: 'mistral',    label: 'Mistral' },
  { value: 'openrouter', label: 'OpenRouter', tooltip: 'Aggregator: one key, 100+ models.' },
  { value: 'perplexity', label: 'Perplexity' },
  { value: 'together',   label: 'Together AI' },
  { value: 'custom',     label: 'Custom URL' },
]

export function EngineTab({ settings, onUpdate }: Props) {
  const [providerInfo, setProviderInfo] = useState<ProviderInfo | null>(null)

  const [localModels, setLocalModels] = useState<LocalModelInfo[]>([])
  const [modelsLoading, setModelsLoading] = useState(true)
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({})
  const [downloading, setDownloading] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)

  // Refresh provider info whenever the active provider settings change
  useEffect(() => {
    api.getActiveProviderInfo().then(setProviderInfo).catch(() => setProviderInfo(null))
  }, [settings.textProcessor, settings.openaiCompatPreset, settings.openaiCompatUrl])

  useEffect(() => {
    invoke<LocalModelInfo[]>('list_local_models')
      .then(setLocalModels)
      .catch(() => {})
      .finally(() => setModelsLoading(false))
  }, [])

  useEffect(() => {
    let unsubs: Array<() => void> = []
    listen<{ model: string; progress: number }>('model-download-progress', e => {
      setDownloadProgress(p => ({ ...p, [e.payload.model]: e.payload.progress }))
      // Re-attach to a download that started before this tab was opened.
      setDownloading(e.payload.progress < 1 ? e.payload.model : null)
    }).then(f => unsubs.push(f))
    listen<string>('model-download-done', e => {
      setDownloading(null)
      setDownloadProgress(p => ({ ...p, [e.payload]: 1 }))
      invoke<LocalModelInfo[]>('list_local_models').then(setLocalModels).catch(() => {})
    }).then(f => unsubs.push(f))
    listen<{ model: string; error: string }>('model-download-error', e => {
      setDownloading(null)
      console.error('Download failed:', e.payload.error)
    }).then(f => unsubs.push(f))
    return () => unsubs.forEach(f => f())
  }, [])

  const refreshModels = () =>
    invoke<LocalModelInfo[]>('list_local_models').then(setLocalModels).catch(() => {})

  const handleDownload = async (modelKey: string) => {
    setDownloading(modelKey)
    setDownloadProgress(p => ({ ...p, [modelKey]: 0 }))
    await invoke('download_local_model', { modelKey })
  }

  const handleDelete = async (modelKey: string) => {
    setDeleting(modelKey)
    try {
      await invoke('delete_local_model', { modelKey })
      if (settings.localModel === modelKey) {
        const remaining = localModels.find(m => m.downloaded && m.key !== modelKey)
        if (remaining) onUpdate({ localModel: remaining.key as LocalModel })
      }
      await refreshModels()
    } catch (e) {
      console.error('Delete failed:', e)
    } finally {
      setDeleting(null)
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>

      {/* ── Transcription engine (Cloud Whisper vs local) ── */}
      <Section title="Transcription">
        <div style={{ display: 'flex', gap: 'var(--sp-2)' }}>
          <EngineBtn
            active={settings.engine === 'cloud'}
            onClick={() => onUpdate({ engine: 'cloud' })}
            title="Cloud (Whisper)"
            desc="Best accuracy · 90+ languages"
          />
          <EngineBtn
            active={settings.engine === 'local'}
            onClick={() => onUpdate({ engine: 'local' })}
            title="Local (Whisper)"
            desc="Fully offline · Private"
          />
        </div>

        {settings.engine === 'cloud' && (
          <div style={{ marginTop: 12 }}>
            <label style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>
              Cloud Whisper provider
            </label>
            <select
              value={settings.cloudTranscriptionProvider}
              onChange={e => onUpdate({
                cloudTranscriptionProvider: e.target.value as CloudTranscriptionProvider,
                cloudTranscriptionModel: null,
              })}
              style={{
                width: '100%', height: 30,
                background: 'var(--bg-secondary)', color: 'var(--text-primary)',
                border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
                padding: '0 8px', fontSize: 'var(--text-sm)', fontFamily: 'inherit',
              }}
            >
              <option value="openai">OpenAI — best accuracy</option>
              <option value="groq">Groq — very fast, free tier</option>
            </select>
            <label style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', display: 'block', margin: '10px 0 4px' }}>
              Transcription model
            </label>
            <select
              value={settings.cloudTranscriptionModel ?? CLOUD_STT_MODELS[settings.cloudTranscriptionProvider][0].id}
              onChange={e => {
                const isDefault = e.target.value === CLOUD_STT_MODELS[settings.cloudTranscriptionProvider][0].id
                onUpdate({ cloudTranscriptionModel: isDefault ? null : e.target.value })
              }}
              style={{
                width: '100%', height: 30,
                background: 'var(--bg-secondary)', color: 'var(--text-primary)',
                border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
                padding: '0 8px', fontSize: 'var(--text-sm)', fontFamily: 'inherit',
              }}
            >
              {CLOUD_STT_MODELS[settings.cloudTranscriptionProvider].map(m => (
                <option key={m.id} value={m.id}>{m.label}</option>
              ))}
            </select>
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 4 }}>
              {settings.cloudTranscriptionProvider === 'groq'
                ? 'Uses your Groq API key (also used for Groq text processing if selected).'
                : 'Uses your OpenAI API key.'}
            </div>
          </div>
        )}
      </Section>

      {/* ── Whisper API key (slot depends on selected cloud Whisper provider) ── */}
      {settings.engine === 'cloud' && (
        <Section title={
          settings.cloudTranscriptionProvider === 'groq'
            ? 'Groq API Key (for Whisper transcription)'
            : 'OpenAI API Key (for Whisper transcription)'
        }>
          <KeyRow
            slot={settings.cloudTranscriptionProvider === 'groq' ? 'compat:groq' : 'openai'}
            description="Stored locally on your device with owner-only permissions — never sent to TypeUrVoice servers."
          />
        </Section>
      )}

      {/* ── Local Whisper models ── */}
      {settings.engine === 'local' && (
        <Section title="Local Models">
          <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', marginBottom: 4 }}>
            Download a Whisper model to transcribe locally — no internet required.
            Multilingual Q5 models support Czech, Slovak and 90+ other languages.
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {localModels.map(model => (
              <ModelRow
                key={model.key}
                model={model}
                active={settings.localModel === model.key}
                progress={downloadProgress[model.key]}
                isDownloading={downloading === model.key}
                isDeleting={deleting === model.key}
                onSelect={() => model.downloaded && onUpdate({ localModel: model.key as LocalModel })}
                onDownload={() => handleDownload(model.key)}
                onDelete={() => handleDelete(model.key)}
              />
            ))}
          </div>
          {localModels.length === 0 && (
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', textAlign: 'center', padding: 12 }}>
              {modelsLoading ? 'Loading model list…' : 'No models available.'}
            </div>
          )}
        </Section>
      )}

      {/* ── Text-processing provider ── */}
      <Section title="Text Processing">
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', marginBottom: 8 }}>
          Choose which AI service handles Clean, Polish, Translate and Summarize modes.
        </div>

        <select
          value={settings.textProcessor}
          onChange={e => onUpdate({ textProcessor: e.target.value as TextProcessor })}
          style={{
            width: '100%', height: 32,
            background: 'var(--bg-secondary)', color: 'var(--text-primary)',
            border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
            padding: '0 8px', fontSize: 'var(--text-sm)', fontFamily: 'inherit',
            cursor: 'pointer',
          }}
        >
          {PROVIDERS.map(p => (
            <option key={p.value} value={p.value}>
              {p.label}{p.sub ? ` — ${p.sub}` : ''}
            </option>
          ))}
        </select>

        {/* OpenAI-compatible: preset picker */}
        {settings.textProcessor === 'openaicompat' && (
          <div style={{ marginTop: 12 }}>
            <label style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>
              Service
            </label>
            <select
              value={settings.openaiCompatPreset}
              onChange={e => onUpdate({
                openaiCompatPreset: e.target.value as OpenAiCompatPreset,
                // Clear processing_model so the new preset's default kicks in
                processingModel: '',
              })}
              style={{
                width: '100%', height: 30,
                background: 'var(--bg-secondary)', color: 'var(--text-primary)',
                border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
                padding: '0 8px', fontSize: 'var(--text-sm)', fontFamily: 'inherit',
              }}
            >
              {COMPAT_PRESETS.map(p => (
                <option key={p.value} value={p.value} title={p.tooltip}>
                  {p.label}
                </option>
              ))}
            </select>
            {COMPAT_PRESETS.find(p => p.value === settings.openaiCompatPreset)?.tooltip && (
              <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 4 }}>
                {COMPAT_PRESETS.find(p => p.value === settings.openaiCompatPreset)!.tooltip}
              </div>
            )}
          </div>
        )}

        {/* Custom URL field (only for OpenAI-compatible → Custom) */}
        {settings.textProcessor === 'openaicompat' && settings.openaiCompatPreset === 'custom' && (
          <div style={{ marginTop: 12 }}>
            <Input
              label="Custom endpoint base URL"
              value={settings.openaiCompatUrl}
              onChange={e => onUpdate({ openaiCompatUrl: e.target.value })}
              placeholder="https://example.com/v1"
              description="TypeUrVoice appends /chat/completions automatically."
            />
          </div>
        )}

        {/* Ollama / LM Studio config */}
        {(settings.textProcessor === 'ollama' || settings.textProcessor === 'lmstudio') && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)', marginTop: 12 }}>
            <Input
              label="Server URL"
              value={settings.textProcessorUrl}
              onChange={e => onUpdate({ textProcessorUrl: e.target.value })}
              placeholder={settings.textProcessor === 'lmstudio' ? 'http://localhost:1234' : 'http://localhost:11434'}
            />
            <Input
              label="Model name"
              value={settings.textProcessorModel}
              onChange={e => onUpdate({ textProcessorModel: e.target.value })}
              placeholder={settings.textProcessor === 'lmstudio' ? 'lmstudio-community/Meta-Llama-3-8B-Instruct-GGUF' : 'gemma4'}
            />
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', lineHeight: 1.5 }}>
              {settings.textProcessor === 'ollama'
                ? 'Make sure Ollama is running: ollama serve'
                : 'Make sure LM Studio is running with the local server enabled.'}
            </div>
          </div>
        )}

        {settings.textProcessor === 'builtin' && (
          <BuiltInLlmPanel settings={settings} onUpdate={onUpdate} />
        )}

        {/* API key (hosted providers only) */}
        {providerInfo?.requiresKey && providerInfo.keySlot && (
          <div style={{ marginTop: 12 }}>
            <KeyRow
              slot={providerInfo.keySlot}
              description={`Stored locally on your device with owner-only permissions. Used only by ${providerInfo.displayName}.`}
            />
          </div>
        )}

        {/* Model picker (hosted providers + Custom) */}
        {providerInfo && settings.textProcessor !== 'ollama' && settings.textProcessor !== 'lmstudio'
          && settings.textProcessor !== 'builtin' && (
          <ModelPicker
            settings={settings}
            providerInfo={providerInfo}
            onUpdate={onUpdate}
          />
        )}
      </Section>
    </div>
  )
}

// ─────────────────────────────────────────────────────────────────────────────
// Sub-components
// ─────────────────────────────────────────────────────────────────────────────

/** API key input + Test button + Saved-key display, scoped to a slot. */
function KeyRow({ slot, description }: { slot: string; description: string }) {
  const [savedKey, setSavedKey] = useState<string | null>(null)
  const [keyInput, setKeyInput] = useState('')
  const [testState, setTestState] = useState<'idle' | 'loading' | 'ok' | 'fail'>('idle')

  const refresh = useCallback(() => {
    api.getApiKey(slot).then(k => setSavedKey(k ?? null))
  }, [slot])

  useEffect(() => { refresh() }, [refresh])

  const handleSave = async () => {
    if (!keyInput.trim()) return
    await api.saveApiKey(slot, keyInput.trim())
    setKeyInput('')
    refresh()
  }

  const handleDelete = async () => {
    await api.deleteApiKey(slot)
    setSavedKey(null)
  }

  const handleTest = async () => {
    const key = keyInput.trim() || savedKey
    if (!key) return
    setTestState('loading')
    try {
      const ok = await api.testApiKey(slot, key)
      setTestState(ok ? 'ok' : 'fail')
    } catch {
      setTestState('fail')
    }
    setTimeout(() => setTestState('idle'), 3000)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)' }}>
      {savedKey ? (
        <div style={{ display: 'flex', gap: 'var(--sp-2)', alignItems: 'center' }}>
          <div style={{
            flex: 1, height: 30, background: 'var(--bg-secondary)',
            border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
            display: 'flex', alignItems: 'center', padding: '0 10px',
            fontSize: 'var(--text-sm)', color: 'var(--text-secondary)',
          }}>
            {maskKey(savedKey)}
          </div>
          <Button variant="ghost" size="sm" onClick={handleDelete}>Remove</Button>
        </div>
      ) : (
        <div style={{ display: 'flex', gap: 'var(--sp-2)' }}>
          <div style={{ flex: 1 }}>
            <Input
              type="password"
              placeholder="Paste your API key"
              value={keyInput}
              onChange={e => setKeyInput(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleSave()}
              description={description}
            />
          </div>
          <Button variant="secondary" size="sm" onClick={handleSave}
            disabled={!keyInput.trim()} style={{ height: 30, alignSelf: 'flex-start' }}>
            Save
          </Button>
        </div>
      )}
      <div style={{ display: 'flex', gap: 'var(--sp-2)', alignItems: 'center' }}>
        <Button variant="ghost" size="sm" onClick={handleTest}
          loading={testState === 'loading'} disabled={!savedKey && !keyInput.trim()}>
          Test connection
        </Button>
        {testState === 'ok' && <StatusBadge ok label="Connected" />}
        {testState === 'fail' && <StatusBadge ok={false} label="Failed — check key & network" />}
      </div>
    </div>
  )
}

/** Truncated display for a saved key. Anthropic keys are sk-ant-..., Gemini is AIza..., etc. */
function maskKey(key: string): string {
  if (key.length <= 8) return '••••'
  return `${key.slice(0, 4)}…${key.slice(-4)}`
}

/** Model selector. Uses dropdown when the provider has a stable catalogue,
 *  otherwise free-form text input (OpenRouter, Together, Custom).
 *  "Refresh" button hits the provider's /models endpoint (24h cache) so users
 *  see new releases without us shipping a code update. */
function ModelPicker({
  settings, providerInfo, onUpdate,
}: { settings: Settings; providerInfo: ProviderInfo; onUpdate: (patch: Partial<Settings>) => void }) {
  const [liveModels, setLiveModels] = useState<string[] | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [refreshError, setRefreshError] = useState<string | null>(null)

  // Reset live cache when provider changes
  useEffect(() => {
    setLiveModels(null)
    setRefreshError(null)
  }, [providerInfo.keySlot])

  const refresh = async () => {
    if (!providerInfo.keySlot) return
    setRefreshing(true)
    setRefreshError(null)
    try {
      const list = await api.fetchProviderModels(providerInfo.keySlot)
      setLiveModels(list)
      if (list.length === 0) {
        setRefreshError('Provider returned no models — check API key.')
      }
    } catch (e) {
      setRefreshError(e instanceof Error ? e.message : String(e))
    } finally {
      setRefreshing(false)
    }
  }

  // Merged list: live (if fetched) ∪ recommended, with recommended first
  const effective: string[] = liveModels && liveModels.length > 0
    ? Array.from(new Set([...providerInfo.recommended, ...liveModels]))
    : providerInfo.recommended

  const value = settings.processingModel || providerInfo.defaultModel
  const inCatalogue = effective.includes(value)
  const useDropdown = providerInfo.stableCatalog && effective.length > 0

  return (
    <div style={{ marginTop: 12 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginBottom: 4 }}>
        <label style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)' }}>
          Model
        </label>
        {providerInfo.requiresKey && (
          <button
            type="button"
            onClick={refresh}
            disabled={refreshing}
            style={{
              fontSize: 'var(--text-xs)',
              color: 'var(--text-tertiary)',
              background: 'transparent',
              border: 'none',
              cursor: refreshing ? 'wait' : 'pointer',
              padding: 0,
              textDecoration: 'underline',
            }}
            title="Fetch the latest model list from the provider (24h cached)"
          >
            {refreshing ? 'Refreshing…' : 'Refresh from provider'}
          </button>
        )}
      </div>

      {useDropdown ? (
        <select
          value={inCatalogue ? value : (effective[0] ?? providerInfo.defaultModel)}
          onChange={e => onUpdate({ processingModel: e.target.value })}
          style={{
            width: '100%', height: 30,
            background: 'var(--bg-secondary)', color: 'var(--text-primary)',
            border: '1px solid var(--border-default)', borderRadius: 'var(--radius)',
            padding: '0 8px', fontSize: 'var(--text-sm)', fontFamily: 'inherit',
            cursor: 'pointer',
          }}
        >
          {effective.map(m => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
      ) : (
        <>
          <Input
            value={value}
            onChange={e => onUpdate({ processingModel: e.target.value })}
            placeholder={providerInfo.defaultModel || 'Enter model identifier'}
          />
          {effective.length > 0 && (
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 4 }}>
              Suggestions: {effective.slice(0, 8).join(', ')}{effective.length > 8 ? `, +${effective.length - 8} more` : ''}
            </div>
          )}
        </>
      )}

      {refreshError && (
        <div role="alert" style={{ color: '#FCA5A5', fontSize: 'var(--text-xs)', marginTop: 4 }}>
          {refreshError}
        </div>
      )}
      {liveModels && !refreshError && (
        <div style={{ color: 'var(--text-tertiary)', fontSize: 'var(--text-xs)', marginTop: 4 }}>
          {liveModels.length} models fetched live (cached for 24h).
        </div>
      )}
    </div>
  )
}

function ModelRow({ model, active, progress, isDownloading, isDeleting, onSelect, onDownload, onDelete }: {
  model: LocalModelInfo
  active: boolean
  progress?: number
  isDownloading: boolean
  isDeleting: boolean
  onSelect: () => void
  onDownload: () => void
  onDelete: () => void
}) {
  const isEnglishOnly = model.key === 'base-en' || model.key === 'tiny-en'
  return (
    <div
      onClick={model.downloaded && !isEnglishOnly ? onSelect : undefined}
      style={{
        display: 'flex', alignItems: 'center', gap: 10,
        padding: '8px 10px', borderRadius: 'var(--radius)',
        border: `1px solid ${active && model.downloaded ? 'var(--border-focus)' : 'var(--border-subtle)'}`,
        background: active && model.downloaded ? 'var(--bg-active)' : 'var(--bg-secondary)',
        opacity: isEnglishOnly ? 0.55 : 1,
        cursor: model.downloaded && !isEnglishOnly ? 'pointer' : 'default',
        transition: 'all var(--t-normal)',
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
          <span style={{ fontSize: 'var(--text-sm)', fontWeight: active ? 500 : 400 }}>
            {model.name}
          </span>
          {isEnglishOnly && (
            <span style={{
              fontSize: 10, padding: '1px 5px', borderRadius: 999,
              background: 'rgba(251, 191, 36, 0.22)', color: 'var(--accent-yellow)',
              fontWeight: 600, letterSpacing: '0.03em',
            }}>EN ONLY</span>
          )}
          {model.downloaded && model.sizeMb > 0 && (
            <span style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>
              {model.sizeMb} MB
            </span>
          )}
        </div>
        {isDownloading && (
          <div style={{ marginTop: 4 }}>
            <div style={{ height: 2, background: 'var(--border-subtle)', borderRadius: 1, overflow: 'hidden' }}>
              <div style={{
                height: '100%', background: 'var(--accent-blue)', borderRadius: 1,
                width: `${Math.round((progress ?? 0) * 100)}%`,
                transition: 'width 300ms ease-out',
              }} />
            </div>
            <div style={{ fontSize: 10, color: 'var(--text-tertiary)', marginTop: 2 }}>
              {Math.round((progress ?? 0) * 100)}%
            </div>
          </div>
        )}
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        {model.downloaded ? (
          <>
            {active && <Check size={12} style={{ color: 'var(--text-primary)' }} />}
            {!active && <HardDrive size={12} style={{ color: 'var(--text-tertiary)' }} />}
            <button
              onClick={e => { e.stopPropagation(); onDelete() }}
              disabled={isDeleting || active}
              title={active ? 'Cannot delete active model' : 'Delete model'}
              style={{
                background: 'none', border: 'none', cursor: isDeleting || active ? 'not-allowed' : 'pointer',
                color: isDeleting || active ? 'var(--text-tertiary)' : '#FF6B6B',
                padding: '2px 4px', borderRadius: 4, display: 'flex', alignItems: 'center',
                opacity: active ? 0.4 : 1, transition: 'opacity var(--t-normal)',
              }}
            >
              <Trash2 size={12} />
            </button>
          </>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            loading={isDownloading}
            onClick={e => { e.stopPropagation(); onDownload() }}
            disabled={isDownloading}
            style={{ padding: '3px 8px', height: 24, fontSize: 11 }}
          >
            {!isDownloading && <Download size={10} />}
            {isDownloading ? 'Downloading…' : 'Download'}
          </Button>
        )}
      </div>
    </div>
  )
}

function StatusBadge({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span style={{
      display: 'flex', alignItems: 'center', gap: 4,
      color: ok ? 'var(--accent-green)' : '#FF6B6B',
      fontSize: 'var(--text-xs)',
    }}>
      {ok ? <CheckCircle size={12} /> : <XCircle size={12} />}
      {label}
    </span>
  )
}

function EngineBtn({ active, onClick, title, desc }: {
  active: boolean; onClick: () => void; title: string; desc: string
}) {
  return (
    <button
      onClick={onClick}
      style={{
        flex: 1, padding: '10px 12px', borderRadius: 'var(--radius-md)',
        border: `1px solid ${active ? 'var(--border-focus)' : 'var(--border-subtle)'}`,
        background: active ? 'var(--bg-active)' : 'var(--bg-secondary)',
        color: 'var(--text-primary)', textAlign: 'left', cursor: 'pointer',
        transition: 'all var(--t-normal)', fontFamily: 'inherit',
      }}
    >
      <div style={{ fontSize: 'var(--text-sm)', fontWeight: 500, marginBottom: 2 }}>{title}</div>
      <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)' }}>{desc}</div>
    </button>
  )
}
