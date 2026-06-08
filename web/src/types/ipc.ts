/** IPC command payloads for Tauri invoke() calls */

export interface ChatRequest {
  message: string
}

export interface ChatResponse {
  reply: string
}

export interface AudioLevelEvent {
  level: number // 0.0 – 1.0
}

export interface EmotionEvent {
  label: 'happy' | 'sad' | 'angry' | 'neutral' | 'surprised' | 'fearful'
  intensity: number
}

export interface AgentStateEvent {
  state: 'idle' | 'thinking' | 'speaking' | 'listening'
}

export interface ToolExecutionEvent {
  tool: string
  status: 'started' | 'completed' | 'failed'
  result?: string
}
