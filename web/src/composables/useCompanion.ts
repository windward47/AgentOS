/**
 * useCompanion — typed composable for all Tauri IPC invoke() calls.
 *
 * Every command is wrapped with explicit parameter types and return types,
 * catching mismatched arguments at compile time instead of runtime.
 */
import { invoke } from '@tauri-apps/api/core'
import type { CompanionConfig, ConversationMeta } from '../types/ipc'

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

  /** Stream chat — fire and forget. Tokens arrive via 'chat_token' events. */
  function chatStream(message: string, history?: { role: string; content: string }[]): Promise<void> {
    return invoke('chat_stream', { message, history: history || [] })
  }

  function clearHistory(): Promise<void> {
    return invoke('clear_history')
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

  function listLive2dModels(): Promise<string[]> {
    return invoke<string[]>('list_live2d_models')
  }

  function setAvatarVisible(visible: boolean): Promise<void> {
    return invoke('set_avatar_visible', { visible })
  }

  function getAvatarVisible(): Promise<boolean> {
    return invoke<boolean>('get_avatar_visible')
  }

  function openFolder(path: string): Promise<void> {
    return invoke('open_folder', { path })
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

  // ── Session / Conversation management ───────────────────────────

  function listConversations(): Promise<{ conversations: ConversationMeta[] }> {
    return invoke<{ conversations: ConversationMeta[] }>('list_conversations')
  }

  function getCurrentConversation(): Promise<{ id: string | null; messages: { role: string; content: string }[] }> {
    return invoke<{ id: string | null; messages: { role: string; content: string }[] }>('get_current_conversation')
  }

  function createConversation(title?: string): Promise<{ conversation: ConversationMeta; id: string }> {
    return invoke<{ conversation: ConversationMeta; id: string }>('create_conversation', { title })
  }

  function switchConversation(id: string): Promise<{ conversation: ConversationMeta; messages: { role: string; content: string }[] }> {
    return invoke<{ conversation: ConversationMeta; messages: { role: string; content: string }[] }>('switch_conversation', { id })
  }

  function deleteConversation(id: string): Promise<{ ok: boolean; currentId: string }> {
    return invoke<{ ok: boolean; currentId: string }>('delete_conversation', { id })
  }

  function renameConversation(id: string, title: string): Promise<{ ok: boolean }> {
    return invoke<{ ok: boolean }>('rename_conversation', { id, title })
  }

  // ── Memory management ───────────────────────────────────────────

  function listMemories(): Promise<{ memories: { id: string; content: string; timestamp: string }[] }> {
    return invoke<{ memories: { id: string; content: string; timestamp: string }[] }>('list_memories')
  }

  function forgetMemory(id: string): Promise<{ ok: boolean }> {
    return invoke<{ ok: boolean }>('forget_memory', { id })
  }

  return {
    getConfig,
    updateConfig,
    chat,
    chatStream,
    clearHistory,
    transcribeAudio,
    synthesizeAudio,
    setLipLevel,
    browseScreenshot,
    listModels,
    downloadModel,
    listLive2dModels,
    setAvatarVisible,
    getAvatarVisible,
    setAvatarAlwaysOnTop,
    setLive2dModel,
    resetAvatarPosition,
    listConversations,
    getCurrentConversation,
    createConversation,
    switchConversation,
    deleteConversation,
    renameConversation,
    listMemories,
    forgetMemory,
  }
}
