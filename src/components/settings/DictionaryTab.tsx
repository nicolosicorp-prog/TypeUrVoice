import { useState } from 'react'
import { X } from 'lucide-react'
import { Input } from '../ui/Input'
import { Button } from '../ui/Button'
import type { Settings } from '../../types'

interface Props {
  settings: Settings
  onUpdate: (patch: Partial<Settings>) => void
}

export function DictionaryTab({ settings, onUpdate }: Props) {
  const [input, setInput] = useState('')

  const addWord = () => {
    const word = input.trim()
    if (!word || settings.dictionary.includes(word)) return
    onUpdate({ dictionary: [...settings.dictionary, word] })
    setInput('')
  }

  const removeWord = (word: string) => {
    onUpdate({ dictionary: settings.dictionary.filter(w => w !== word) })
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-4)' }}>
      <div>
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600, marginBottom: 10 }}>Custom Words</div>
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', marginBottom: 12 }}>
          Add technical terms, names, and brand words to improve transcription accuracy.
        </div>
        <div style={{ display: 'flex', gap: 'var(--sp-2)' }}>
          <div style={{ flex: 1 }}>
            <Input
              placeholder="e.g. Kubernetes, TypeUrVoice"
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && addWord()}
            />
          </div>
          <Button
            variant="secondary"
            size="sm"
            onClick={addWord}
            disabled={!input.trim()}
            style={{ height: 30, alignSelf: 'flex-start' }}
          >
            Add
          </Button>
        </div>
      </div>

      {settings.dictionary.length === 0 ? (
        <div style={{
          padding: '20px 16px', borderRadius: 'var(--radius-md)',
          border: '1px dashed var(--border-subtle)', textAlign: 'center',
        }}>
          <div style={{ fontSize: 'var(--text-sm)', color: 'var(--text-tertiary)' }}>
            Your dictionary is empty.
          </div>
          <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-tertiary)', marginTop: 4 }}>
            Add custom words to improve transcription accuracy.
          </div>
        </div>
      ) : (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {settings.dictionary.map(word => (
            <div
              key={word}
              style={{
                display: 'flex', alignItems: 'center', gap: 4,
                padding: '3px 8px', borderRadius: 999,
                background: 'var(--bg-tertiary)', border: '1px solid var(--border-default)',
                fontSize: 'var(--text-xs)', color: 'var(--text-primary)',
              }}
            >
              {word}
              <button
                onClick={() => removeWord(word)}
                style={{
                  background: 'none', border: 'none', color: 'var(--text-tertiary)',
                  cursor: 'pointer', padding: 0, display: 'flex', marginLeft: 2,
                }}
              >
                <X size={10} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
