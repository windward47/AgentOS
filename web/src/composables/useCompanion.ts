/**
 * useCompanion — typed composable for all Tauri IPC invoke() calls.
 *
 * Every command is wrapped with explicit parameter types and return types,
 * catching mismatched arguments at compile time instead of runtime.
 */
import { invoke } from '@tauri-apps/api/core'
import type { CompanionConfig } from '../types/ipc'

// ── Composable ───────────────────────────────────────────────────────

export function useCompanion() {
  // ── Config ────────────────────────────────────────────────────────

  function getConfig(): Promise<CompanionConfig> {
    return invoke<CompanionConfig>('get_config')
  }

  function updateConfig(newConfig: CompanionConfig): Promise<void> {
    return invoke('update_config', { newConfig })
  }

  // ── Chat & Audio ──────────────────────────────────────────────────

  function chat(message: string): Promise<string> {
    return invoke<string>('chat', { message })
  }

  /** S3.3: Stream chat — returns immediately, tokens arrive via 'chat_token' Tauri events. */
  function chatStream(message: string): Promise<void> {
    return invoke('chat_stream', { message })
  }

  function transcribeAudio(audio: number[]): Promise<string> {
    return invoke<string>('transcribe_audio', { audio })
  }

  function synthesizeAudio(text: string, voice?: string): Promise<number[]> {
    return invoke<number[]>('synthesize_audio', { text, voice })
  }

  function setLipLevel(level: number): Promise<void> {
    return invoke('set_lip_level', { level })
  }

  // ── Tools ─────────────────────────────────────────────────────────

  function browseScreenshot(url: string): Promise<string> {
    return invoke<string>('browse_screenshot', { url })
  }

  function listModels(baseUrl: string, apiKey: string): Promise<string[]> {
    return invoke<string[]>('list_models', { baseUrl, apiKey })
  }

  // ── Live2D ────────────────────────────────────────────────────────

  function downloadModel(url: string, modelId: string): Promise<void> {
    return invoke('cmd_download_model', { url, modelId })
  }

  // ── B1d: unified event bus ────────────────────────────────────────

  function sendAction(type: string, payload: Record<string, unknown> = {}): Promise<Record<string, unknown>> {
    return invoke<Record<string, unknown>>('agent_action', { actionType: type, payload })
  }

  function listLive2dModels(): Promise<string[]> {
    return invoke<string[]>('list_live2d_models')
  }

  function setAvatarVisible(visible: boolean): Promise<void> {
    return invoke('set_avatar_visible', { visible })
  }

  function getAvatarVisible(): Promise<boolean> {
    return invoke<boolean>('get_avatar_visible')
  }

  function setAvatarAlwaysOnTop(onTop: boolean): Promise<void> {
    return invoke('set_avatar_always_on_top', { onTop })
  }

  function setLive2dModel(modelPath: string): Promise<void> {
    return invoke('set_live2d_model', { modelPath })
  }

  function resetAvatarPosition(): Promise<void> {
    return invoke('reset_avatar_position')
  }

  return {
    getConfig,
    updateConfig,
    chat,
    chatStream,
    transcribeAudio,
    synthesizeAudio,
    setLipLevel,
    browseScreenshot,
    listModels,
    downloadModel,
    sendAction,
    listLive2dModels,
    setAvatarVisible,
    getAvatarVisible,
    setAvatarAlwaysOnTop,
    setLive2dModel,
    resetAvatarPosition,
  }
}
