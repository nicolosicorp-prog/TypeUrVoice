import { ReactNode, useRef } from 'react'
import type { TabId } from '../../types'

interface Tab { id: TabId; label: string; icon: ReactNode }

interface Props {
  tabs: Tab[]
  active: TabId
  onChange: (id: TabId) => void
}

export function Tabs({ tabs, active, onChange }: Props) {
  const btnRefs = useRef<Record<string, HTMLButtonElement | null>>({})

  const focusByIndex = (i: number) => {
    const clamped = (i + tabs.length) % tabs.length
    const tab = tabs[clamped]
    const el = btnRefs.current[tab.id]
    el?.focus()
    onChange(tab.id)
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>, idx: number) => {
    switch (e.key) {
      case 'ArrowDown':
      case 'ArrowRight':
        e.preventDefault(); focusByIndex(idx + 1); break
      case 'ArrowUp':
      case 'ArrowLeft':
        e.preventDefault(); focusByIndex(idx - 1); break
      case 'Home':
        e.preventDefault(); focusByIndex(0); break
      case 'End':
        e.preventDefault(); focusByIndex(tabs.length - 1); break
    }
  }

  return (
    <nav
      role="tablist"
      aria-orientation="vertical"
      aria-label="Settings sections"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 2,
        padding: '8px 6px',
        width: 140,
        flexShrink: 0,
        borderRight: '1px solid var(--border-subtle)',
        background: 'var(--bg-secondary)',
      }}
    >
      {tabs.map((tab, idx) => {
        const selected = active === tab.id
        return (
          <button
            key={tab.id}
            ref={el => { btnRefs.current[tab.id] = el }}
            role="tab"
            id={`tab-${tab.id}`}
            aria-selected={selected}
            aria-controls={`tabpanel-${tab.id}`}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(tab.id)}
            onKeyDown={e => onKeyDown(e, idx)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 'var(--sp-2)',
              padding: '6px 8px',
              borderRadius: 'var(--radius)',
              border: 'none',
              background: selected ? 'var(--bg-active)' : 'transparent',
              color: selected ? 'var(--text-primary)' : 'var(--text-secondary)',
              fontSize: 'var(--text-sm)',
              fontFamily: 'inherit',
              fontWeight: selected ? 500 : 400,
              cursor: 'pointer',
              textAlign: 'left',
              transition: 'background var(--t-normal), color var(--t-normal)',
            }}
            onMouseEnter={e => {
              if (!selected) e.currentTarget.style.background = 'var(--bg-hover)'
            }}
            onMouseLeave={e => {
              if (!selected) e.currentTarget.style.background = 'transparent'
            }}
          >
            <span style={{ opacity: selected ? 1 : 0.6, display: 'flex' }}>{tab.icon}</span>
            {tab.label}
          </button>
        )
      })}
    </nav>
  )
}
