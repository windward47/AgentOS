/** IPC command payloads for Tauri invoke() calls */

export interface ChatRequest { message: string }
export interface ChatResponse { reply: string }
export interface AudioLevelEvent { level: number } // 0.0 – 1.0
export type EmotionLabel = 'happy' | 'sad' | 'angry' | 'neutral' | 'surprised' | 'fearful'
export interface EmotionEvent { label: EmotionLabel; intensity: number }
export interface AgentStateEvent { state: 'idle' | 'thinking' | 'speaking' | 'listening' }
export interface ToolExecutionEvent { tool: string; status: 'started' | 'completed' | 'failed'; result?: string }

// ── Config types (1:1 with Rust CompanionConfig) ──

export interface ProviderConfig {
  provider: string
  url: string | null
  key: string | null
  model: string | null
}

export interface GlobalVoiceConfig {
  record_hotkey: string
  tts_hotkey: string
  inject_mode_switch_hotkey: string
  engine_switch_hotkey: string
  inject_mode: string
  asr_engine: string
  tts_engine: string
}

export interface CompanionConfig {
  sandbox_path: string
  llm_provider: string
  asr_provider: string
  tts_provider: string
  system_mode: boolean
  tts_auto_play: boolean
  vad_threshold: number
  voice_mode: string
  tts_voice: string
  tts_speed: number
  user_name: string
  custom_system_prompt: string | null
  api_token: string | null
  llm: ProviderConfig
  asr: ProviderConfig
  tts: ProviderConfig
  custom_providers: ProviderConfig[]
  global_voice: GlobalVoiceConfig
}
