import { useState, useEffect, useCallback, useRef } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Check, X } from 'lucide-react'
import { useTauriEvent } from '../../hooks/useTauriEvent'
import type { RecordingState } from '../../types'

export function RecordingOverlay() {
  const [state, setState] = useState<RecordingState>('idle')
  const [elapsed, setElapsed] = useState(0)
  const [errorMsg, setErrorMsg] = useState('')
  // Live-preview text (local engine) — streamed while the user speaks.
  const [liveText, setLiveText] = useState('')
  // Final processed text, shown briefly after the paste in place of the preview.
  const [finalText, setFinalText] = useState('')
  const finalRef = useRef('')

  // Hide the native window when idle, show when active
  useEffect(() => {
    const win = getCurrentWindow()
    if (state === 'idle') {
      win.hide().catch(() => {})
    }
  }, [state])

  const onStateChange = useCallback((s: string) => {
    setState(s as RecordingState)
    if (s === 'recording') { setElapsed(0); setLiveText(''); setFinalText(''); finalRef.current = '' }
    if (s === 'done' || s === 'error') setLiveText('')
    if (s === 'done') {
      setTimeout(() => setState('idle'), finalRef.current ? 3500 : 1500)
    }
    if (s === 'error') {
      setTimeout(() => setState('idle'), 3000)
    }
  }, [])

  useTauriEvent<string>('recording-state', onStateChange)
  useTauriEvent<string>('recording-error', (msg) => { setErrorMsg(msg) })
  useTauriEvent<string>('partial-transcript', (text) => { setLiveText(text) })
  useTauriEvent<string>('final-transcript', (text) => { finalRef.current = text; setFinalText(text) })

  // Safety: if Processing never reports back, hide the bubble. Only for
  // Processing — a recording may legitimately run for minutes, and the
  // bubble is the user's only sign that it still is (this used to be a flat
  // 60 s for every state, which hid the bubble mid-dictation). 380 s covers
  // the backend's 360 s pipeline timeout, which always ends in done/error.
  useEffect(() => {
    if (state !== 'processing') return
    const safety = setTimeout(() => setState('idle'), 380_000)
    return () => clearTimeout(safety)
  }, [state])

  // Timer while recording
  useEffect(() => {
    if (state !== 'recording') return
    const t = setInterval(() => setElapsed(s => s + 1), 1000)
    return () => clearInterval(t)
  }, [state])

  const formatTime = (s: number) =>
    `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`

  if (state === 'idle') return null

  // Expanded card once live text exists (window is resized by the backend).
  const showFinal = finalText !== '' && state === 'done'
  const showLive = showFinal || (liveText !== '' && (state === 'recording' || state === 'processing'))
  const shown = showFinal ? finalText : liveText
  // Show the tail so the newest words are always visible.
  const tail = shown.length > LIVE_TAIL_CHARS
    ? '…' + shown.slice(shown.length - LIVE_TAIL_CHARS).replace(/^\S*\s/, '')
    : shown

  return (
    <div style={{
      position: 'fixed', inset: 0,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      background: 'transparent',
    }}>
      <div style={{
        display: 'flex',
        flexDirection: showLive ? 'column' : 'row',
        alignItems: showLive ? 'stretch' : 'center',
        gap: showLive ? 6 : 10,
        padding: showLive ? '10px 16px' : '8px 16px',
        width: showLive ? 'calc(100% - 16px)' : undefined,
        boxSizing: 'border-box',
        borderRadius: showLive ? 16 : 999,
        background: 'rgba(28, 28, 30, 0.88)',
        border: '1px solid rgba(255,255,255,0.1)',
        backdropFilter: 'blur(24px)',
        WebkitBackdropFilter: 'blur(24px)',
        boxShadow: '0 4px 24px rgba(0,0,0,0.5), 0 0 0 0.5px rgba(255,255,255,0.05)',
        animation: 'slideUp 180ms ease-out',
        minWidth: 120,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        {state === 'recording' && (
          <>
            <div style={{
              width: 8, height: 8, borderRadius: '50%',
              background: '#FF3B30',
              boxShadow: '0 0 6px rgba(255, 59, 48, 0.6)',
              animation: 'pulse 1.4s ease-in-out infinite',
              flexShrink: 0,
            }} />
            <span style={{
              fontSize: 13, fontWeight: 500,
              // Recordings are capped at 5 min (audio/capture.rs) — warn
              // in the last 30 s, then say the cap was reached.
              color: elapsed >= MAX_RECORDING_SECS - 30 ? '#FF9F0A' : '#fff',
              fontVariantNumeric: 'tabular-nums', letterSpacing: '0.02em',
            }}>
              {elapsed >= MAX_RECORDING_SECS ? '5:00 max reached' : formatTime(elapsed)}
            </span>
            <Waveform />
          </>
        )}

        {state === 'processing' && (
          <>
            <div style={{
              width: 14, height: 14, borderRadius: '50%',
              border: '2px solid rgba(255,255,255,0.15)',
              borderTopColor: '#fff',
              animation: 'spin 0.7s linear infinite',
              flexShrink: 0,
            }} />
            <span style={{ fontSize: 12, color: 'rgba(255,255,255,0.7)' }}>Processing...</span>
          </>
        )}

        {state === 'done' && (
          <>
            <Check size={14} style={{ color: '#34D399', flexShrink: 0 }} strokeWidth={2.5} />
            <span style={{ fontSize: 12, color: '#fff' }}>Done</span>
          </>
        )}

        {state === 'error' && (
          <>
            <X size={14} style={{ color: '#FF6B6B', flexShrink: 0 }} strokeWidth={2.5} />
            <span style={{ fontSize: 11, color: '#FF6B6B', maxWidth: 180, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {errorMsg || 'Failed'}
            </span>
          </>
        )}
        </div>

        {showLive && (
          <div
            aria-live="polite"
            style={{
              fontSize: 13,
              lineHeight: '18px',
              color: state === 'processing' ? 'rgba(255,255,255,0.6)' : '#fff',
              display: '-webkit-box',
              WebkitLineClamp: 2,
              WebkitBoxOrient: 'vertical',
              overflow: 'hidden',
              animation: 'fadeIn 160ms ease-out',
            }}
          >
            {tail}
          </div>
        )}
      </div>
    </div>
  )
}

/** Matches MAX_RECORDING_SECS in src-tauri/src/audio/capture.rs. */
const MAX_RECORDING_SECS = 300

/** ~2 lines of 13px text in the 460px-wide bubble. */
const LIVE_TAIL_CHARS = 110

function Waveform() {
  return (
    <div style={{ display: 'flex', gap: 2, alignItems: 'center', height: 14, marginLeft: 2 }}>
      {[0.35, 0.7, 0.45, 1.0, 0.55, 0.85, 0.35].map((h, i) => (
        <div
          key={i}
          style={{
            width: 2,
            height: `${h * 100}%`,
            background: 'rgba(255,255,255,0.45)',
            borderRadius: 1,
            animation: `pulse ${0.6 + i * 0.12}s ease-in-out infinite alternate`,
          }}
        />
      ))}
    </div>
  )
}
