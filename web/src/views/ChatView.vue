<script setup lang="ts">
import { ref, nextTick, watch, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/app'

const store = useAppStore()
const { messages } = store

const input = ref('')
const list = ref<HTMLDivElement>()
const shortcutDisplay = ref('Ctrl+Shift+V')

// ── TTS settings (shared with Pinia) ──
const ttsAuto = ref(true)
const ttsVoice = ref('茉莉')
const ttsSpeed = ref(1.0)
const voices = ['mimo_default', '冰糖', '茉莉', '苏打', '白桦', 'Mia', 'Chloe', 'Milo', 'Dean']

// ── Voice input ──
const voiceMode = ref<'dictation' | 'chat'>('chat')
const recording = ref(false)
const vadLevel = ref(0)
let mediaRecorder: MediaRecorder | null = null
let audioChunks: Blob[] = []
let analyserNode: AnalyserNode | null = null

async function startRecording() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    const audioCtx = new AudioContext()
    const source = audioCtx.createMediaStreamSource(stream)
    analyserNode = audioCtx.createAnalyser()
    analyserNode.fftSize = 256
    source.connect(analyserNode)

    // VAD monitoring loop
    const vadData = new Uint8Array(analyserNode.frequencyBinCount)
    const vadLoop = () => {
      if (!analyserNode || !recording.value) return
      analyserNode.getByteTimeDomainData(vadData)
      let sum = 0
      for (let i = 0; i < vadData.length; i++) {
        const v = (vadData[i] - 128) / 128
        sum += v * v
      }
      vadLevel.value = Math.sqrt(sum / vadData.length) * 3
      if (recording.value) requestAnimationFrame(vadLoop)
    }
    requestAnimationFrame(vadLoop)

    mediaRecorder = new MediaRecorder(stream, { mimeType: 'audio/webm' })
    audioChunks = []
    mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) audioChunks.push(e.data) }
    mediaRecorder.onstop = async () => {
      stream.getTracks().forEach(t => t.stop())
      analyserNode = null
      vadLevel.value = 0
      if (audioChunks.length === 0) return
      const blob = new Blob(audioChunks, { type: 'audio/webm' })
      const pcm = await blobToPCM(blob)
      if (pcm.length === 0) return
      try {
        const text = await invoke<string>('transcribe_audio', { audio: Array.from(pcm) })
        if (!text) return
        if (voiceMode.value === 'chat') {
          store.setSending(true)
          store.addMessage({ role: 'user', content: text })
          try {
            const reply = await invoke<string>('chat', { message: text })
            store.addMessage({ role: 'assistant', content: reply })
          } catch (err: any) { store.addMessage({ role: 'assistant', content: String(err) }) }
          finally { store.setSending(false) }
        } else {
          input.value = input.value ? input.value + text : text
        }
      } catch (err: any) { console.error('ASR error:', err) }
    }
    mediaRecorder.start()
    recording.value = true
  } catch (err: any) { console.error('Mic error:', err) }
}

function stopRecording() {
  if (mediaRecorder && mediaRecorder.state === 'recording') mediaRecorder.stop()
  recording.value = false
  analyserNode = null
  vadLevel.value = 0
}
function toggleRecord() { recording.value ? stopRecording() : startRecording() }
function toggleVoiceMode() { voiceMode.value = voiceMode.value === 'chat' ? 'dictation' : 'chat' }

// ── Hotkey ──
const hotkey = ref('Ctrl+Shift+V')
let savedHotkey = hotkey.value

function parseHotkey(e: KeyboardEvent): string | null {
  if (['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement)?.tagName)) return null
  const parts: string[] = []
  if (e.ctrlKey || e.metaKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey && !(e.key.length === 1 && e.key === e.key.toUpperCase())) parts.push('Shift')
  const key = e.key === ' ' ? 'Space' : e.key.length === 1 ? e.key.toUpperCase() : e.key
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(key)) return null
  parts.push(key)
  return parts.join('+')
}

function onKeyDown(e: KeyboardEvent) {
  const parsed = parseHotkey(e)
  if (parsed === savedHotkey) {
    e.preventDefault()
    toggleRecord()
  }
}

function startHotkeyCapture() {
  const original = hotkey.value
  shortcutDisplay.value = 'Press keys...'
  const handler = (e: KeyboardEvent) => {
    const p = parseHotkey(e)
    if (p) {
      e.preventDefault()
      savedHotkey = p
      hotkey.value = p
      shortcutDisplay.value = p
      document.removeEventListener('keydown', handler)
    }
  }
  document.addEventListener('keydown', handler)
  setTimeout(() => {
    hotkey.value = original
    shortcutDisplay.value = original
    document.removeEventListener('keydown', handler)
  }, 5000)
}

onMounted(() => {
  document.addEventListener('keydown', onKeyDown)
})
onBeforeUnmount(() => {
  document.removeEventListener('keydown', onKeyDown)
})

// ── TTS with lip sync ──
const playingId = ref<number | null>(null)
let audioCtx: AudioContext | null = null
let lipTimer: ReturnType<typeof setInterval> | null = null
let ttsLastAutoMs = 0

async function playTTS(text: string, msgIdx: number) {
  if (playingId.value === msgIdx) { stopTTS(); return }
  stopTTS()
  playingId.value = msgIdx
  try {
    const pcm = await invoke<number[]>('synthesize_audio', { text: text.slice(0, 300), voice: ttsVoice.value })
    if (!pcm || pcm.length === 0) { playingId.value = null; return }
    if (!audioCtx) audioCtx = new AudioContext()
    const tgtRate = ttsSpeed.value
    const buf = audioCtx.createBuffer(1, pcm.length, Math.round(16000 / tgtRate))
    const ch = buf.getChannelData(0)
    for (let i = 0; i < pcm.length; i++) ch[i] = pcm[i]
    const src = audioCtx.createBufferSource()
    src.buffer = buf; src.playbackRate.value = tgtRate
    src.connect(audioCtx.destination)
    src.onended = () => { playingId.value = null; if (lipTimer) clearInterval(lipTimer) }
    src.start()

    let elapsed = 0
    lipTimer = setInterval(() => {
      elapsed += 60
      const idx = Math.floor((elapsed / 1000) * 16000)
      if (idx >= pcm.length) { if (lipTimer) clearInterval(lipTimer); return }
      const start = Math.max(0, idx - 480), end = Math.min(pcm.length, idx + 480)
      let sum = 0; for (let i = start; i < end; i++) sum += pcm[i] * pcm[i]
      const rms = Math.sqrt(sum / (end - start))
      emit('audio_level', { level: Math.min(rms * 3, 1.0) }).catch(() => {})
    }, 60)
  } catch (err: any) { console.error('TTS error:', err); playingId.value = null }
}

function stopTTS() {
  if (lipTimer) { clearInterval(lipTimer); lipTimer = null }
  if (audioCtx) { try { audioCtx.close() } catch {}; audioCtx = null }
  playingId.value = null
}

// ── Chat ──
async function send() {
  const text = input.value.trim()
  if (!text || store.sending) return
  input.value = ''; store.setSending(true)
  store.addMessage({ role: 'user', content: text })
  try {
    const reply = await invoke<string>('chat', { message: text })
    store.addMessage({ role: 'assistant', content: reply })
  } catch (err: any) { store.addMessage({ role: 'assistant', content: String(err) }) }
  finally { store.setSending(false) }
}

// Auto-TTS: play new assistant messages automatically
watch(() => messages.length, async (len) => {
  await nextTick(); if (list.value) list.value.scrollTop = list.value.scrollHeight
  if (ttsAuto.value && len > 0) {
    const last = messages[len - 1]
    if (last.role === 'assistant' && Date.now() - ttsLastAutoMs > 2000) {
      ttsLastAutoMs = Date.now()
      playTTS(last.content, len - 1)
    }
  }
})

async function blobToPCM(blob: Blob): Promise<Float32Array> {
  const ctx = new OfflineAudioContext(1, 1, 16000)
  const audioBuf = await ctx.decodeAudioData(await blob.arrayBuffer())
  return audioBuf.getChannelData(0)
}
</script>

<template>
  <div class="flex flex-col h-full min-h-0">
    <!-- Header -->
    <div class="flex items-center justify-between px-5 h-14 border-b border-gray-100 shrink-0">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-emerald-400" />
        <span class="text-sm font-medium text-gray-700">Companion</span>
      </div>
      <div class="flex items-center gap-3">
        <!-- Hotkey display -->
        <div class="flex items-center gap-1.5 text-[11px] text-gray-400">
          <span>PTT:</span>
          <button @click="startHotkeyCapture"
            class="px-1.5 py-0.5 rounded border border-gray-200 font-mono text-[10px] hover:border-blue-300 transition-colors"
            :title="shortcutDisplay !== hotkey ? 'Press new hotkey...' : 'Click to change hotkey'">
            {{ shortcutDisplay }}
          </button>
        </div>
        <button @click="store.clearMessages()"
          class="text-xs text-gray-400 hover:text-gray-600 px-2 py-1 rounded-md hover:bg-gray-50 transition-colors">Clear</button>
      </div>
    </div>

    <!-- Messages -->
    <div ref="list" class="flex-1 overflow-y-auto px-5">
      <div class="max-w-[720px] mx-auto py-6 space-y-6">
        <div v-if="messages.length === 0" class="flex flex-col items-center justify-center py-20 text-gray-400">
          <div class="w-12 h-12 rounded-full bg-gray-100 flex items-center justify-center mb-4 text-2xl">🤖</div>
          <p class="text-base font-medium text-gray-500 mb-1">Companion</p>
          <p class="text-sm text-gray-400">Type or press {{ hotkey }} to speak</p>
        </div>

        <template v-for="(m, i) in messages" :key="i">
          <div v-if="m.role === 'user'" class="flex justify-end">
            <div class="max-w-[75%] rounded-2xl rounded-br-md bg-blue-500 text-white px-4 py-2.5 text-[15px] leading-relaxed whitespace-pre-wrap">{{ m.content }}</div>
          </div>
          <div v-else class="flex gap-3 group">
            <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-400 to-purple-500 flex items-center justify-center text-white text-xs shrink-0 mt-0.5">AI</div>
            <div class="flex flex-col gap-1">
              <div class="max-w-[75%] rounded-2xl rounded-bl-md bg-gray-50 border border-gray-100 px-4 py-2.5 text-[15px] leading-relaxed text-gray-800 whitespace-pre-wrap">{{ m.content }}</div>
              <button @click="playTTS(m.content, i)"
                :class="['text-[11px] self-start px-2 py-0.5 rounded-full border transition-colors', playingId === i ? 'bg-emerald-50 border-emerald-200 text-emerald-600' : 'text-gray-300 border-transparent hover:text-gray-500 hover:border-gray-200']">
                {{ playingId === i ? '⏹ Stop' : '🔊 Listen' }}
              </button>
            </div>
          </div>
        </template>

        <div v-if="store.sending" class="flex gap-3">
          <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-400 to-purple-500 flex items-center justify-center text-white text-xs shrink-0 mt-0.5">AI</div>
          <div class="rounded-2xl rounded-bl-md bg-gray-50 border border-gray-100 px-5 py-3 flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce" /><span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce [animation-delay:0.15s]" /><span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce [animation-delay:0.3s]" />
          </div>
        </div>
      </div>
    </div>

    <!-- Bottom bar: Voice + TTS controls -->
    <div class="border-t border-gray-100 px-4 py-3 shrink-0">
      <div class="max-w-[720px] mx-auto space-y-2">
        <!-- Row 1: Voice mode + TTS auto + speed -->
        <div class="flex items-center gap-3 flex-wrap">
          <span class="text-[11px] text-gray-400">Voice:</span>
          <button @click="toggleVoiceMode"
            :class="['text-[11px] px-2 py-0.5 rounded-full border transition-colors', voiceMode === 'chat' ? 'bg-blue-50 border-blue-200 text-blue-600' : 'text-gray-400 border-gray-200 hover:border-gray-300']">
            {{ voiceMode === 'chat' ? '💬 Chat' : '📝 Dictate' }}
          </button>

          <span class="text-[11px] text-gray-400 ml-2">TTS:</span>
          <button @click="ttsAuto = !ttsAuto"
            :class="['text-[11px] px-2 py-0.5 rounded-full border transition-colors', ttsAuto ? 'bg-emerald-50 border-emerald-200 text-emerald-600' : 'text-gray-400 border-gray-200 hover:border-gray-300']">
            {{ ttsAuto ? '🔊 Auto' : '🔇 Manual' }}
          </button>

          <select v-model="ttsVoice"
            class="text-[11px] px-2 py-0.5 rounded-full border border-gray-200 bg-transparent text-gray-500 outline-none">
            <option v-for="v in voices" :key="v" :value="v">{{ v }}</option>
          </select>

          <div class="flex items-center gap-1">
            <span class="text-[11px] text-gray-400">Speed:</span>
            <input v-model.number="ttsSpeed" type="range" min="0.5" max="2.0" step="0.1"
              class="w-16 h-1 accent-blue-500" />
            <span class="text-[11px] text-gray-400 w-6">{{ ttsSpeed.toFixed(1) }}</span>
          </div>
        </div>

        <!-- Row 2: Input bar -->
        <div class="flex items-center gap-2 bg-gray-50 border border-gray-200 rounded-2xl px-4 py-1 focus-within:border-blue-300 focus-within:ring-2 focus-within:ring-blue-100 transition-all">
          <!-- Mic button with VAD indicator -->
          <button @click="toggleRecord"
            :class="['shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm transition-colors relative', recording ? 'bg-red-100 text-red-500' : 'text-gray-400 hover:text-gray-600 hover:bg-gray-100']"
            :title="recording ? 'Stop recording (or ' + hotkey + ')' : 'Start recording (' + hotkey + ')'">
            <span v-if="recording" class="text-xs">⏹</span>
            <span v-else>🎤</span>
            <!-- VAD ring -->
            <div v-if="recording" class="absolute inset-0 rounded-full border-2 transition-all duration-75"
              :class="vadLevel > 0.3 ? 'border-red-400' : 'border-gray-300'"
              :style="{ transform: `scale(${1 + vadLevel * 0.4})`, opacity: 0.3 + vadLevel * 0.5 }" />
          </button>

          <input v-model="input" type="text"
            :placeholder="voiceMode === 'chat' ? 'Message or ' + hotkey + ' to speak...' : 'Message or ' + hotkey + ' to dictate...'"
            class="flex-1 bg-transparent py-2.5 text-[15px] outline-none placeholder:text-gray-400 text-gray-800"
            @keydown.enter="send" :disabled="store.sending" />
          <button @click="send" :disabled="store.sending || !input.trim()"
            class="shrink-0 rounded-xl bg-blue-500 hover:bg-blue-600 disabled:opacity-30 disabled:hover:bg-blue-500 text-white px-4 py-1.5 text-sm font-medium transition-colors">Send</button>
        </div>
      </div>
    </div>
  </div>
</template>
