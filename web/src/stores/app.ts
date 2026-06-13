import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

export const useAppStore = defineStore('app', () => {
  const messages = ref<ChatMessage[]>([])
  const isConnected = ref(false)
  const sending = ref(false)

  function addMessage(msg: ChatMessage) {
    messages.value.push(msg)
    console.log('[store] addMessage role=', msg.role, 'count=', messages.value.length, 'allRoles=', messages.value.map(m => m.role))
  }

  function clearMessages() {
    messages.value = []
  }

  function setSending(v: boolean) {
    sending.value = v
  }

  return { messages, isConnected, sending, addMessage, clearMessages, setSending }
})
