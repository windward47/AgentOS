<script setup lang="ts">
import { ref, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import Live2DCanvas from '../components/Live2DCanvas.vue'

export interface Message {
  role: 'user' | 'assistant' | 'system'
  content: string
}

const messages = ref<Message[]>([])
const input = ref('')
const sending = ref(false)
const chatContainer = ref<HTMLElement | null>(null)

async function sendMessage() {
  const text = input.value.trim()
  if (!text || sending.value) return

  input.value = ''
  sending.value = true
  messages.value.push({ role: 'user', content: text })

  try {
    const reply = await invoke<string>('chat', { message: text })
    messages.value.push({ role: 'assistant', content: reply })
  } catch (err) {
    messages.value.push({
      role: 'assistant',
      content: `Error: ${err}`,
    })
  } finally {
    sending.value = false
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    sendMessage()
  }
}

watch(
  () => messages.value.length,
  async () => {
    await nextTick()
    if (chatContainer.value) {
      chatContainer.value.scrollTop = chatContainer.value.scrollHeight
    }
  },
)
</script>

<template>
  <div class="flex h-full">
    <div class="hidden md:flex flex-col items-center justify-center w-[300px] min-w-[300px] border-r border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800">
      <Live2DCanvas />
    </div>

    <div class="flex-1 flex flex-col min-w-0 bg-white dark:bg-gray-900">
      <div class="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div class="flex items-center gap-2">
          <span class="w-2 h-2 rounded-full bg-green-500" title="Agent ready"></span>
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">Companion</span>
        </div>
        <div class="flex items-center gap-3">
          <RouterLink to="/settings" class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors">Settings</RouterLink>
          <button @click="messages = []" class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors">Clear</button>
        </div>
      </div>

      <div
        ref="chatContainer"
        class="flex-1 overflow-y-auto px-4 py-4 space-y-4"
      >
        <div
          v-for="(msg, i) in messages"
          :key="i"
          class="flex"
          :class="msg.role === 'user' ? 'justify-end' : 'justify-start'"
        >
          <div
            class="max-w-[80%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap"
            :class="
              msg.role === 'user'
                ? 'bg-blue-500 text-white rounded-br-md'
                : msg.role === 'system'
                  ? 'bg-gray-100 dark:bg-gray-800 text-gray-500 italic rounded-bl-md'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-800 dark:text-gray-200 rounded-bl-md'
            "
          >
            {{ msg.content }}
          </div>
        </div>

        <div v-if="sending" class="flex justify-start">
          <div class="bg-gray-100 dark:bg-gray-800 rounded-2xl rounded-bl-md px-4 py-3">
            <div class="flex gap-1.5">
              <span class="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style="animation-delay: 0ms"></span>
              <span class="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style="animation-delay: 150ms"></span>
              <span class="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style="animation-delay: 300ms"></span>
            </div>
          </div>
        </div>

        <div
          v-if="messages.length === 0 && !sending"
          class="flex flex-col items-center justify-center h-full text-gray-400 dark:text-gray-500"
        >
          <div class="text-5xl mb-4">&#x1F916;</div>
          <p class="text-lg font-medium">Companion</p>
          <p class="text-sm mt-1">Type a message to start</p>
        </div>
      </div>

      <div class="border-t border-gray-200 dark:border-gray-700 px-4 py-3">
        <div class="flex gap-2">
          <input
            v-model="input"
            type="text"
            placeholder="Type a message..."
            class="flex-1 px-4 py-2.5 rounded-xl border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 text-sm outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all"
            @keydown="handleKeydown"
            :disabled="sending"
          />
          <button
            @click="sendMessage"
            :disabled="sending || !input.trim()"
            class="px-5 py-2.5 rounded-xl bg-blue-500 text-white text-sm font-medium disabled:opacity-40 hover:bg-blue-600 active:scale-95 transition-all"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
