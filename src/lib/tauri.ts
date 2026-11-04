import { invoke } from '@tauri-apps/api/core'
import type { Settings, HistoryEntry, PermissionsState, ProviderInfo, OccupationProfile, BuiltinLlmModel, BuiltinLlmStatus } from '../types'

export const api = {
  getSettings: () => invoke<Settings>('get_settings'),
  updateSettings: (settings: Settings) => invoke<void>('update_settings', { newSettings: settings }),

  /** Slot examples: "openai", "anthropic", "gemini", "compat:grok", "compat:groq", … */
  saveApiKey: (slot: string, key: string) => invoke<void>('save_api_key', { slot, key }),
  getApiKey: (slot: string) => invoke<string | null>('get_api_key', { slot }),
  deleteApiKey: (slot: string) => invoke<void>('delete_api_key', { slot }),
  testApiKey: (slot: string, key: string) => invoke<boolean>('test_api_key', { slot, key }),

  /** Metadata for the currently-selected text-processing provider. */
  getActiveProviderInfo: () => invoke<ProviderInfo>('get_active_provider_info'),

  /** Live /models fetch for a provider slot (24h disk-cached). Empty list if
   *  the provider has no /models endpoint (Perplexity) or the key is missing. */
  fetchProviderModels: (slot: string) =>
    invoke<string[]>('fetch_provider_models', { slot }),

  checkPermissions: () => invoke<PermissionsState>('check_permissions'),
  openPermissionSettings: (permission: string) =>
    invoke<void>('open_permission_settings', { permission }),

  getHistory: (limit = 50, offset = 0) =>
    invoke<HistoryEntry[]>('get_history', { limit, offset }),
  deleteHistoryEntry: (id: number) => invoke<void>('delete_history_entry', { id }),
  clearHistory: () => invoke<void>('clear_history'),

  checkForUpdates(): Promise<{ available: boolean; version: string | null; notes: string | null }> {
    return invoke('check_for_updates')
  },

  listInputDevices: () => invoke<string[]>('list_input_devices'),
  showCrashLog: () => invoke<void>('show_crash_log'),
  knockAvailable: () => invoke<boolean>('knock_available'),

  // Built-in (on-device) LLM
  listBuiltinLlmModels: () => invoke<BuiltinLlmModel[]>('list_builtin_llm_models'),
  downloadBuiltinLlmModel: (modelKey: string) => invoke<void>('download_builtin_llm_model', { modelKey }),
  deleteBuiltinLlmModel: (modelKey: string) => invoke<void>('delete_builtin_llm_model', { modelKey }),
  getBuiltinLlmStatus: () => invoke<BuiltinLlmStatus>('get_builtin_llm_status'),
  unloadBuiltinLlm: () => invoke<void>('unload_builtin_llm'),

  /** Built-in + user-cloned professional profiles. */
  listOccupationProfiles: () =>
    invoke<OccupationProfile[]>('list_occupation_profiles'),
  /** Pass `null` to clear; pass an id to activate that profile. Returns the
   *  resolved profile so the UI can confirm the activation succeeded. */
  setActiveOccupationProfile: (profileId: string | null) =>
    invoke<OccupationProfile | null>('set_active_occupation_profile', { profileId }),
  saveUserOccupationProfile: (profile: OccupationProfile) =>
    invoke<void>('save_user_occupation_profile', { profile }),
  deleteUserOccupationProfile: (profileId: string) =>
    invoke<void>('delete_user_occupation_profile', { profileId }),
}
