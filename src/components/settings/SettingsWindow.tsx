import { useState } from 'react'
import { Settings as SettingsIcon, Cpu, AlignLeft, Languages, FileText, BookOpen, Clock, HelpCircle } from 'lucide-react'
import { Tabs } from '../ui/Tabs'
import { GeneralTab } from './GeneralTab'
import { EngineTab } from './EngineTab'
import { OutputTab } from './OutputTab'
import { TranslationTab } from './TranslationTab'
import { SummaryTab } from './SummaryTab'
import { DictionaryTab } from './DictionaryTab'
import { HistoryTab } from './HistoryTab'
import { HelpTab } from './HelpTab'
import { useSettings } from '../../hooks/useSettings'
import { useTauriEvent } from '../../hooks/useTauriEvent'
import { Toast } from '../ui/Toast'
import type { TabId } from '../../types'

const TABS = [
  { id: 'general' as TabId, label: 'General', icon: <SettingsIcon size={14} /> },
  { id: 'engine' as TabId, label: 'Engine', icon: <Cpu size={14} /> },
  { id: 'output' as TabId, label: 'Output', icon: <AlignLeft size={14} /> },
  { id: 'translation' as TabId, label: 'Translation', icon: <Languages size={14} /> },
  { id: 'summary' as TabId, label: 'Summary', icon: <FileText size={14} /> },
  { id: 'dictionary' as TabId, label: 'Dictionary', icon: <BookOpen size={14} /> },
  { id: 'history' as TabId, label: 'History', icon: <Clock size={14} /> },
  { id: 'help' as TabId, label: 'Help', icon: <HelpCircle size={14} /> },
]

export function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<TabId>('general')
  const { settings, update, loading, lastError, clearError } = useSettings()

  // Navigate to specific tab via event (e.g. from tray "Recent Activity")
  useTauriEvent<string>('navigate', (tab) => {
    if (TABS.some(t => t.id === tab)) {
      setActiveTab(tab as TabId)
    }
  })

  if (loading) return <LoadingScreen />

  return (
    <div style={{
      display: 'flex',
      height: '100vh',
      background: 'var(--bg-primary)',
      overflow: 'hidden',
    }}>
      <Tabs tabs={TABS} active={activeTab} onChange={setActiveTab} />

      <div
        role="tabpanel"
        id={`tabpanel-${activeTab}`}
        aria-labelledby={`tab-${activeTab}`}
        style={{
          flex: 1,
          padding: '20px 20px',
          overflowY: 'auto',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        <div style={{ marginBottom: 16 }}>
          <h2 style={{ fontSize: 'var(--text-lg)', fontWeight: 600, color: 'var(--text-primary)' }}>
            {TABS.find(t => t.id === activeTab)?.label}
          </h2>
        </div>

        {activeTab === 'general' && <GeneralTab settings={settings} onUpdate={update} />}
        {activeTab === 'engine' && <EngineTab settings={settings} onUpdate={update} />}
        {activeTab === 'output' && <OutputTab settings={settings} onUpdate={update} />}
        {activeTab === 'translation' && <TranslationTab settings={settings} onUpdate={update} />}
        {activeTab === 'summary' && <SummaryTab settings={settings} onUpdate={update} />}
        {activeTab === 'dictionary' && <DictionaryTab settings={settings} onUpdate={update} />}
        {activeTab === 'history' && <HistoryTab settings={settings} onUpdate={update} />}
        {activeTab === 'help' && <HelpTab />}
      </div>
      <Toast message={lastError} onDismiss={clearError} />
    </div>
  )
}

function LoadingScreen() {
  return (
    <div style={{
      height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center',
      background: 'var(--bg-primary)',
    }}>
      <div style={{ width: 16, height: 16, borderRadius: '50%', border: '2px solid var(--border-default)', borderTopColor: 'var(--text-primary)' }} className="animate-spin" />
    </div>
  )
}
