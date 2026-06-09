<script setup lang="ts">
import { ref, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../stores/app'

const store = useAppStore()
const { messages } = store
const input = ref('')
const list = ref<HTMLDivElement>()

// ── Voice input ──
const voiceMode = ref<'dictation' | 'chat'>('chat') // dictation = insert text, chat = auto-send
const recording = ref(false)
const voiceLabel = ref('🎤')
let mediaRecorder: MediaRecorder | null = null
let audioChunks: Blob[] = []

async function startRecording() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    mediaRecorder = new MediaRecorder(stream, { mimeType: 'audio/webm' })
    audioChunks = []

    mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) audioChunks.push(e.data) }

    mediaRecorder.onstop = async () => {
      stream.getTracks().forEach(t => t.stop())
      if (audioChunks.length === 0) return

      const blob = new Blob(audioChunks, { type: 'audio/webm' })
      const pcm = await blobToPCM(blob)
      if (pcm.length === 0) return

      try {
        const text = await invoke<string>('transcribe_audio', { audio: Array.from(pcm) })
        if (!text) return
        if (voiceMode.value === 'chat') {
          // Auto-send as chat message
          store.setSending(true)
          store.addMessage({ role: 'user', content: text })
          try {
            const reply = await invoke<string>('chat', { message: text })
            store.addMessage({ role: 'assistant', content: reply })
          } catch (err: any) {
            store.addMessage({ role: 'assistant', content: String(err) })
          } finally { store.setSending(false) }
        } else {
          // Insert at cursor
          input.value = input.value ? input.value + text : text
        }
      } catch (err: any) {
        console.error('ASR error:', err)
      }
    }

    mediaRecorder.start()
    recording.value = true
    voiceLabel.value = '⏹'
  } catch (err: any) {
    console.error('Mic error:', err)
  }
}

function stopRecording() {
  if (mediaRecorder && mediaRecorder.state === 'recording') {
    mediaRecorder.stop()
  }
  recording.value = false
  voiceLabel.value = '🎤'
}

function toggleRecord() {
  recording.value ? stopRecording() : startRecording()
}

function toggleVoiceMode() {
  voiceMode.value = voiceMode.value === 'chat' ? 'dictation' : 'chat'
}

// ── TTS playback ──
const playingId = ref<number | null>(null)
let audioCtx: AudioContext | null = null

async function playTTS(text: string, msgIdx: number) {
  if (playingId.value === msgIdx) {
    stopTTS()
    return
  }
  stopTTS()
  playingId.value = msgIdx

  try {
    const pcm = await invoke<number[]>('synthesize_audio', { text: text.slice(0, 200), voice: '茉莉' })
    if (!pcm || pcm.length === 0) return

    if (!audioCtx) audioCtx = new AudioContext()
    const buffer = audioCtx.createBuffer(1, pcm.length, 16000)
    const channel = buffer.getChannelData(0)
    for (let i = 0; i < pcm.length; i++) channel[i] = pcm[i]

    const source = audioCtx.createBufferSource()
    source.buffer = buffer
    source.connect(audioCtx.destination)
    source.onended = () => { playingId.value = null }
    source.start()
  } catch (err: any) {
    console.error('TTS error:', err)
    playingId.value = null
  }
}

function stopTTS() {
  if (audioCtx) { audioCtx.close(); audioCtx = null }
  playingId.value = null
}

// ── Audio conversion ──
async function blobToPCM(blob: Blob): Promise<Float32Array> {
  const ctx = new OfflineAudioContext(1, 1, 16000)
  const arrayBuf = await blob.arrayBuffer()
  const audioBuf = await ctx.decodeAudioData(arrayBuf)
  const pcm = audioBuf.getChannelData(0)
  return pcm
}

// ── Chat ──
async function send() {
  const text = input.value.trim()
  if (!text || store.sending) return
  input.value = ''
  store.setSending(true)
  store.addMessage({ role: 'user', content: text })
  try {
    const reply = await invoke<string>('chat', { message: text })
    store.addMessage({ role: 'assistant', content: reply })
  } catch (err: any) {
    store.addMessage({ role: 'assistant', content: String(err) })
  } finally { store.setSending(false) }
}

watch(() => messages.length, async () => {
  await nextTick(); if (list.value) list.value.scrollTop = list.value.scrollHeight
})
</script>

<template>
  <div class="flex flex-col h-full min-h-0">
    <!-- Header -->
    <div class="flex items-center justify-between px-5 h-14 border-b border-gray-100 shrink-0">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-emerald-400" />
        <span class="text-sm font-medium text-gray-700">Companion</span>
      </div>
      <div class="flex items-center gap-2">
        <button @click="store.clearMessages()" class="text-xs text-gray-400 hover:text-gray-600 px-2 py-1 rounded-md hover:bg-gray-50 transition-colors">Clear</button>
      </div>
    </div>

    <!-- Messages -->
    <div ref="list" class="flex-1 overflow-y-auto px-5">
      <div class="max-w-[720px] mx-auto py-6 space-y-6">
        <div v-if="messages.length === 0" class="flex flex-col items-center justify-center py-20 text-gray-400">
          <div class="w-12 h-12 rounded-full bg-gray-100 flex items-center justify-center mb-4 text-2xl">🤖</div>
          <p class="text-base font-medium text-gray-500 mb-1">Companion</p>
          <p class="text-sm text-gray-400">Type or use 🎤 to speak</p>
        </div>

        <template v-for="(m, i) in messages" :key="i">
          <div v-if="m.role === 'user'" class="flex justify-end">
            <div class="max-w-[75%] rounded-2xl rounded-br-md bg-blue-500 text-white px-4 py-2.5 text-[15px] leading-relaxed whitespace-pre-wrap">{{ m.content }}</div>
          </div>
          <div v-else class="flex gap-3 group">
            <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-400 to-purple-500 flex items-center justify-center text-white text-xs shrink-0 mt-0.5">AI</div>
            <div class="flex flex-col gap-1">
              <div class="max-w-[75%] rounded-2xl rounded-bl-md bg-gray-50 border border-gray-100 px-4 py-2.5 text-[15px] leading-relaxed text-gray-800 whitespace-pre-wrap">{{ m.content }}</div>
              <button
                @click="playTTS(m.content, i)"
                :class="[
                  'text-[11px] self-start px-2 py-0.5 rounded-full border transition-colors',
                  playingId === i
                    ? 'bg-emerald-50 border-emerald-200 text-emerald-600'
                    : 'text-gray-300 border-transparent hover:text-gray-500 hover:border-gray-200'
                ]"
              >
                {{ playingId === i ? '⏹ Stop' : '🔊 Listen' }}
              </button>
            </div>
          </div>
        </template>

        <div v-if="store.sending" class="flex gap-3">
          <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-400 to-purple-500 flex items-center justify-center text-white text-xs shrink-0 mt-0.5">AI</div>
          <div class="rounded-2xl rounded-bl-md bg-gray-50 border border-gray-100 px-5 py-3 flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce" />
            <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce [animation-delay:0.15s]" />
            <span class="w-2 h-2 rounded-full bg-gray-300 animate-bounce [animation-delay:0.3s]" />
          </div>
        </div>
      </div>
    </div>

    <!-- Input bar -->
    <div class="border-t border-gray-100 px-4 py-3 shrink-0">
      <div class="max-w-[720px] mx-auto space-y-2">
        <!-- Voice mode toggle -->
        <div class="flex items-center gap-2">
          <span class="text-[11px] text-gray-400">Voice mode:</span>
          <button
            @click="toggleVoiceMode"
            :class="[
              'text-[11px] px-2 py-0.5 rounded-full border transition-colors',
              voiceMode === 'chat'
                ? 'bg-blue-50 border-blue-200 text-blue-600'
                : 'text-gray-400 border-gray-200 hover:border-gray-300'
            ]"
          >
            {{ voiceMode === 'chat' ? '💬 Real-time chat' : '📝 Dictation' }}
          </button>
        </div>

        <div class="flex items-center gap-2 bg-gray-50 border border-gray-200 rounded-2xl px-4 py-1 focus-within:border-blue-300 focus-within:ring-2 focus-within:ring-blue-100 transition-all">
          <!-- Mic button -->
          <button
            @click="toggleRecord"
            :class="[
              'shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm transition-colors',
              recording
                ? 'bg-red-100 text-red-500 animate-pulse'
                : 'text-gray-400 hover:text-gray-600 hover:bg-gray-100'
            ]"
            :title="recording ? 'Stop recording' : 'Start recording'"
          >
            {{ voiceLabel }}
          </button>

          <input v-model="input" type="text"
                 :placeholder="voiceMode === 'chat' ? 'Message or 🎤 speak...' : 'Message or 🎤 dictate...'"
                 class="flex-1 bg-transparent py-2.5 text-[15px] outline-none placeholder:text-gray-400 text-gray-800"
                 @keydown.enter="send" :disabled="store.sending" />

          <button @click="send" :disabled="store.sending || !input.trim()"
                  class="shrink-0 rounded-xl bg-blue-500 hover:bg-blue-600 disabled:opacity-30 disabled:hover:bg-blue-500 text-white px-4 py-1.5 text-sm font-medium transition-colors">Send</button>
        </div>
      </div>
    </div>
  </div>
</template>
