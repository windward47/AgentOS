import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ConversationMeta } from '../types/ipc'

export interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

export const useAppStore = defineStore('app', () => {
  const messages = ref<ChatMessage[]>([])
  const conversations = ref<ConversationMeta[]>([])
  const currentConversationId = ref<string | null>(null)
  const isConnected = ref(false)
  const sending = ref(false)

  // ── Message actions ───────────────────────────────────────────────

  function addMessage(msg: ChatMessage) {
    messages.value.push(msg)
  }

  function setMessages(msgs: ChatMessage[]) {
    messages.value = msgs
  }

  function clearMessages() {
    messages.value = []
  }

  function setSending(v: boolean) {
    sending.value = v
  }

  // ── Conversation actions ──────────────────────────────────────────

  function setConversations(list: ConversationMeta[]) {
    conversations.value = list
  }

  function setCurrentConversationId(id: string | null) {
    currentConversationId.value = id
  }

  function patchConversation(id: string, patch: Partial<ConversationMeta>) {
    const idx = conversations.value.findIndex(c => c.id === id)
    if (idx >= 0) {
      conversations.value[idx] = { ...conversations.value[idx], ...patch }
    }
  }

  function removeConversation(id: string) {
    conversations.value = conversations.value.filter(c => c.id !== id)
  }

  return {
    messages,
    conversations,
    currentConversationId,
    isConnected,
    sending,
    addMessage,
    setMessages,
    clearMessages,
    setSending,
    setConversations,
    setCurrentConversationId,
    patchConversation,
    removeConversation,
  }
})
