<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useCompanion } from '../composables/useCompanion'
import { useAppStore } from '../stores/app'
import { useTtsPlayer } from '../composables/useTtsPlayer'
import MarkdownRenderer from '../components/MarkdownRenderer.vue'

const { chatStream, getCurrentConversation, listConversations, clearHistory, getConfig, synthesizeAudio, setLipLevel, updateConfig, openFolder } = useCompanion()
const store = useAppStore()
const input = ref('')
const sandboxPath = ref('')
const showSandbox = ref(false)

// ── TTS ──
const ttsAuto = ref(false)
const ttsVoice = ref('茉莉')
const ttsSpeed = ref(1.0)
const voices = ['mimo_default', '冰糖', '茉莉', '苏打', '白桦', 'Mia', 'Chloe', 'Milo', 'Dean']
const tts = useTtsPlayer({ ttsVoice, ttsSpeed, voiceState: ref('idle'), synthesizeAudio, setLipLevel })
const { playingId, playTTS, stopTTS } = tts

/** Strip markdown for TTS playback */
function stripForTTS(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, '').replace(/`([^`]+)`/g, '$1')
    .replace(/[*_~]{1,2}/g, '').replace(/!?\[.*?\]\(.*?\)/g, '')
    .replace(/^[#>*-]\s?/gm, '').replace(/\n{3,}/g, '\n\n').trim()
}

// ── Persist TTS settings ──
watch([ttsAuto, ttsVoice, ttsSpeed], async () => {
  try { const c = await getConfig(); c.tts_auto_play = ttsAuto.value; c.tts_voice = ttsVoice.value; c.tts_speed = ttsSpeed.value; await updateConfig(c) } catch {}
})

// ── Single streaming listener ──
let sendResolve: (() => void) | null = null
let _l = false; if (!_l) { _l = true; import('@tauri-apps/api/event').then(m => {
  m.listen<{ token?: string; done?: boolean }>('chat_token', (evt) => {
    const p = evt.payload
    if (p.done) {
      store.setSending(false)
      if (p.token) {
        const msgs = store.messages
        for (let i = msgs.length - 1; i >= 0; i--) {
          if (msgs[i].role === 'assistant' && !msgs[i].content) { msgs[i].content = p.token; break }
        }
      }
      if (sendResolve) { sendResolve(); sendResolve = null }
      // Auto-TTS on done
      if (ttsAuto.value) {
        const idx = store.messages.length - 1
        const last = store.messages[idx]
        if (last?.role === 'assistant' && last.content) {
          playTTS(stripForTTS(last.content), idx).catch(() => {})
        }
      }
      return
    }
    if (p.token) {
      const msgs = store.messages
      for (let i = msgs.length - 1; i >= 0; i--) {
        if (msgs[i].role === 'assistant') { msgs[i].content += p.token; return }
      }
      store.addMessage({ role: 'assistant', content: p.token })
    }
  })
}) }

// ── Send ──
function send() {
  const msg = input.value.trim()
  if (!msg || store.sending) return
  input.value = ''

  store.setSending(true)
  store.addMessage({ role: 'user', content: msg })
  store.addMessage({ role: 'assistant', content: '' })

  const history = store.messages.slice(0, -2).map(m => ({ role: m.role, content: m.content }))
  // Fire and await done via shared promise
  new Promise<void>((resolve) => { sendResolve = resolve }).then(() => {})
  chatStream(msg, history).catch((err: any) => {
    if (sendResolve) { sendResolve(); sendResolve = null }
    store.setSending(false)
    store.addMessage({ role: 'assistant', content: `⚠️ ${err}` })
  })
}

async function doClear() {
  store.clearMessages()
  try { await clearHistory() } catch {}
}

onMounted(async () => {
  try {
    const c = await getConfig()
    showSandbox.value = !c.system_mode
    sandboxPath.value = c.sandbox_path
    if (c.tts_auto_play !== undefined) ttsAuto.value = c.tts_auto_play
    if (c.tts_voice) ttsVoice.value = c.tts_voice
    if (c.tts_speed) ttsSpeed.value = c.tts_speed
  } catch {}
  try {
    const [convList, current] = await Promise.all([listConversations(), getCurrentConversation()])
    store.setConversations(convList.conversations)
    if (current.id) {
      store.setCurrentConversationId(current.id)
      store.setMessages(current.messages
        .filter((m: any) => !(m.role === 'assistant' && !m.content))
        .map((m: any) => ({ role: m.role as 'user' | 'assistant', content: m.content })))
    }
  } catch {}
})
</script>

<template>
  <div class="flex flex-col h-full min-h-0">
    <div class="flex items-center justify-between px-5 h-14 border-b border-gray-100 shrink-0">
      <span class="text-sm font-medium text-gray-700">Companion</span>
      <div class="flex items-center gap-2">
        <button v-if="showSandbox" @click="openFolder(sandboxPath)" class="text-xs text-amber-500 hover:text-amber-600 px-2 py-1 rounded-md hover:bg-amber-50 transition-colors" title="Open sandbox folder">📂 Sandbox</button>
        <button @click="doClear" class="text-xs text-gray-400 hover:text-gray-600">Clear</button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-5">
      <div class="max-w-[720px] mx-auto py-6 space-y-6">
        <div v-if="store.messages.length === 0" class="text-center py-20 text-gray-400 text-sm">Type a message to start</div>
        <template v-for="(m, i) in store.messages" :key="i">
          <div v-if="m.role === 'user'" class="flex justify-end">
            <div class="max-w-[75%] rounded-2xl rounded-br-md bg-blue-500 text-white px-4 py-2.5 text-[15px] whitespace-pre-wrap">{{ m.content }}</div>
          </div>
          <div v-else class="flex gap-3">
            <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-400 to-purple-500 flex items-center justify-center text-white text-xs shrink-0">AI</div>
            <div class="max-w-[75%] rounded-2xl rounded-bl-md bg-gray-50 border border-gray-100 px-4 py-2.5 text-gray-800">
              <MarkdownRenderer v-if="m.content" :content="m.content" />
              <span v-else class="flex gap-1">
                <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce"></span>
                <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce" style="animation-delay:0.15s"></span>
                <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce" style="animation-delay:0.3s"></span>
              </span>
            </div>
            <button v-if="m.content" @click.stop="playTTS(stripForTTS(m.content), i)" :class="['text-[10px] self-start px-2 py-0.5 rounded-full border transition-colors', playingId === i ? 'bg-emerald-50 border-emerald-200 text-emerald-600' : 'text-gray-300 border-transparent hover:text-gray-500 hover:border-gray-200']">{{ playingId === i ? '⏹' : '🔊' }}</button>
          </div>
        </template>
      </div>
    </div>

    <div class="border-t border-gray-100 px-4 py-3 shrink-0">
      <div class="max-w-[720px] mx-auto space-y-2">
        <div class="flex items-center gap-3 flex-wrap">
          <label class="flex items-center gap-1 text-[11px] text-gray-500 cursor-pointer">
            <input type="checkbox" v-model="ttsAuto" class="w-3 h-3" /> Auto TTS
          </label>
          <select v-model="ttsVoice" class="text-[11px] px-2 py-0.5 rounded-full border border-gray-200 bg-transparent text-gray-500 outline-none">
            <option v-for="v in voices" :key="v" :value="v">{{ v }}</option>
          </select>
          <div class="flex items-center gap-1">
            <span class="text-[11px] text-gray-400">Speed:</span>
            <input v-model.number="ttsSpeed" type="range" min="0.5" max="2.0" step="0.1" class="w-16 h-1 accent-blue-500" />
            <span class="text-[11px] text-gray-400 w-6">{{ ttsSpeed.toFixed(1) }}</span>
          </div>
        </div>
        <div class="flex items-center gap-2 bg-gray-50 border border-gray-200 rounded-2xl px-4 py-1 focus-within:border-blue-300 focus-within:ring-2 focus-within:ring-blue-100 transition-all">
          <input v-model="input" type="text" placeholder="Message..." class="flex-1 bg-transparent py-2.5 text-[15px] outline-none" @keydown.enter="(e) => { if (!e.isComposing) send() }" :disabled="store.sending" />
          <button @click="send" :disabled="store.sending || !input.trim()" class="shrink-0 rounded-xl bg-blue-500 hover:bg-blue-600 disabled:opacity-30 text-white px-4 py-1.5 text-sm font-medium">Send</button>
        </div>
      </div>
    </div>
  </div>
</template>
