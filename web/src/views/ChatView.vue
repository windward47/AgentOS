<script setup lang="ts">
import { ref, nextTick, watch, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../stores/app'

const store = useAppStore()
const { messages } = store

const input = ref('')
const list = ref<HTMLDivElement>()
const shortcutDisplay = ref('Ctrl+Shift+V')

// ── TTS settings ──
const ttsAuto = ref(true)
const ttsVoice = ref('茉莉')
const ttsSpeed = ref(1.0)
const voiceMode = ref<'ptt' | 'auto'>('ptt')
const voices = ['mimo_default', '冰糖', '茉莉', '苏打', '白桦', 'Mia', 'Chloe', 'Milo', 'Dean']

// ── Toast notifications ──
const toast = ref('')

function showToast(msg: string) {
  toast.value = msg
  setTimeout(() => { if (toast.value === msg) toast.value = '' }, 4000)
}

// ── Interrupt sensitivity ──
const interruptEnabled = ref(true)
const interruptSensitivity = ref(0.3)

// ── Voice input ──
const recording = ref(false)
// ── Auto-stop VAD state ──
let autoVadRaf = 0
let autoSpeechStart = 0
let autoSilenceStart = 0
const AUTO_MIN_SPEECH_MS = 400    // must speak ≥ this to qualify for auto-stop
const AUTO_SILENCE_MS = 1300      // silence after speech → stop
const vadLevel = ref(0)
let mediaRecorder: MediaRecorder | null = null
let audioChunks: Blob[] = []
let analyserNode: AnalyserNode | null = null
let backgroundStream: MediaStream | null = null
let bgAnalyser: AnalyserNode | null = null
let bgVadLoop = 0

async function startRecording() {
  try {
    const stream = backgroundStream || await navigator.mediaDevices.getUserMedia({ audio: true })
    if (!backgroundStream) backgroundStream = stream
    const audioCtx = new AudioContext()
    const source = audioCtx.createMediaStreamSource(stream)
    analyserNode = audioCtx.createAnalyser()
    analyserNode.fftSize = 256
    source.connect(analyserNode)

    // VAD monitoring loop (display + auto-stop)
    const vadData = new Uint8Array(analyserNode.frequencyBinCount)
    const isAuto = voiceMode.value === 'auto'
    const threshold = 0.04  // RMS energy threshold for "speaking"
    autoSpeechStart = 0
    autoSilenceStart = 0

    const vadLoop = () => {
      if (!analyserNode || !recording.value) { autoVadRaf = 0; return }
      analyserNode.getByteTimeDomainData(vadData)
      let sum = 0
      for (let i = 0; i < vadData.length; i++) {
        const v = (vadData[i] - 128) / 128
        sum += v * v
      }
      const rms = Math.sqrt(sum / vadData.length) * 3
      vadLevel.value = rms

      // Auto-stop logic
      if (isAuto) {
        const now = Date.now()
        if (rms > threshold) {
          // Speaking
          if (!autoSpeechStart) autoSpeechStart = now
          autoSilenceStart = 0
        } else {
          // Silence — only auto-stop if user spoke enough first
          if (autoSpeechStart && (now - autoSpeechStart) >= AUTO_MIN_SPEECH_MS) {
            if (!autoSilenceStart) autoSilenceStart = now
            if ((now - autoSilenceStart) >= AUTO_SILENCE_MS) {
              stopRecording()
              return
            }
          } else {
            autoSpeechStart = 0
          }
        }
      }

      if (recording.value) autoVadRaf = requestAnimationFrame(vadLoop)
    }
    autoVadRaf = requestAnimationFrame(vadLoop)

    mediaRecorder = new MediaRecorder(stream, { mimeType: 'audio/webm' })
    audioChunks = []
    mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) audioChunks.push(e.data) }
    mediaRecorder.onstop = async () => {
      analyserNode = null
      vadLevel.value = 0
      if (audioChunks.length === 0) return
      const blob = new Blob(audioChunks, { type: 'audio/webm' })
      const pcm = await blobToPCM(blob)
      if (pcm.length === 0) return
      try {
        const text = await invoke<string>('transcribe_audio', { audio: Array.from(pcm) })
        if (!text) return
        store.setSending(true)
        store.addMessage({ role: 'user', content: text })
        try {
          const reply = await invoke<string>('chat', { message: text })
          store.addMessage({ role: 'assistant', content: reply })
        } catch (err: any) { store.addMessage({ role: 'assistant', content: String(err) }) }
        finally { store.setSending(false) }
      } catch (err: any) { showToast('ASR: ' + String(err)); console.error('ASR error:', err) }
    }
    mediaRecorder.start()
    recording.value = true
  } catch (err: any) { showToast('Mic: ' + String(err)); console.error('Mic error:', err) }
}

function stopRecording() {
  if (autoVadRaf) { cancelAnimationFrame(autoVadRaf); autoVadRaf = 0 }
  if (mediaRecorder && mediaRecorder.state === 'recording') mediaRecorder.stop()
  recording.value = false
  analyserNode = null
  vadLevel.value = 0
}

// ── Interrupt: background mic monitoring ──
let bgAudioCtx: AudioContext | null = null

async function startBackgroundVAD() {
  if (bgAnalyser) return
  try {
    if (!backgroundStream) {
      backgroundStream = await navigator.mediaDevices.getUserMedia({ audio: true })
    }
    bgAudioCtx = new AudioContext()
    const src = bgAudioCtx.createMediaStreamSource(backgroundStream)
    bgAnalyser = bgAudioCtx.createAnalyser()
    bgAnalyser.fftSize = 256
    src.connect(bgAnalyser)

    const data = new Uint8Array(bgAnalyser.frequencyBinCount)
    const loop = () => {
      if (!bgAnalyser || !interruptEnabled.value) { bgVadLoop = requestAnimationFrame(loop); return }
      bgAnalyser.getByteTimeDomainData(data)
      // Background VAD active — keeps mic stream alive for interrupts
      bgVadLoop = requestAnimationFrame(loop)
    }
    bgVadLoop = requestAnimationFrame(loop)
  } catch (e) { /* mic denied, no interrupt */ }
}

onMounted(() => {
  startBackgroundVAD()
  // Load voice mode from config
  invoke<{ voice_mode: string }>('get_config').then(c => {
    if (c.voice_mode === 'auto' || c.voice_mode === 'ptt') voiceMode.value = c.voice_mode
  }).catch(() => {})
})

onBeforeUnmount(() => {
  cancelAnimationFrame(bgVadLoop)
  bgAnalyser = null
  bgAudioCtx?.close()
  backgroundStream?.getTracks().forEach(t => t.stop())
})

function toggleRecord() { recording.value ? stopRecording() : startRecording() }


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
    hotkey.value = original; shortcutDisplay.value = original
    document.removeEventListener('keydown', handler)
  }, 5000)
}

onMounted(() => { document.addEventListener('keydown', onKeyDown) })
onBeforeUnmount(() => { document.removeEventListener('keydown', onKeyDown) })

// ── TTS with lip sync + interrupt ──
const playingId = ref<number | null>(null)
let audioCtx: AudioContext | null = null
let lipTimer: ReturnType<typeof setInterval> | null = null
let ttsLastAutoMs = 0
let ttsSource: AudioBufferSourceNode | null = null

async function playTTS(text: string, msgIdx: number) {
  if (playingId.value === msgIdx) { stopTTS(); return }
  stopTTS()
  playingId.value = msgIdx
  try {
    const textForTTS = text.length > 300 ? text.slice(0, 300) + '…' : text
    const pcm = await invoke<number[]>('synthesize_audio', { text: textForTTS, voice: ttsVoice.value })
    if (!pcm || pcm.length === 0) { playingId.value = null; return }
    if (!audioCtx) audioCtx = new AudioContext()
    const buf = audioCtx.createBuffer(1, pcm.length, 16000)
    const ch = buf.getChannelData(0)
    for (let i = 0; i < pcm.length; i++) ch[i] = pcm[i]
    ttsSource = audioCtx.createBufferSource()
    ttsSource.buffer = buf
    ttsSource.playbackRate.value = ttsSpeed.value
    ttsSource.connect(audioCtx.destination)
    ttsSource.onended = () => { playingId.value = null; if (lipTimer) clearInterval(lipTimer); ttsSource = null }
    ttsSource.start()

    let elapsed = 0
    lipTimer = setInterval(() => {
      elapsed += 60
      const idx = Math.floor((elapsed / 1000) * 16000)
      if (idx >= pcm.length) { if (lipTimer) clearInterval(lipTimer); return }
      const start = Math.max(0, idx - 480), end = Math.min(pcm.length, idx + 480)
      let sum = 0; for (let i = start; i < end; i++) sum += pcm[i] * pcm[i]
      const rms = Math.sqrt(sum / (end - start))
      invoke('set_lip_level', { level: Math.min(rms * 3, 1.0) }).catch(() => {})
    }, 60)
  } catch (err: any) { showToast('TTS: ' + String(err)); console.error('TTS error:', err); playingId.value = null }
}

function stopTTS() {
  if (ttsSource) { try { ttsSource.stop() } catch {}; ttsSource = null }
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

// ── Interrupt: background VAD with dual threshold ──
const interruptSpeechMs = ref(300)     // ms of sustained voice to trigger
const interruptSilenceMs = ref(800)    // ms of silence after speech to stop recording
let interruptRecording = false
let interruptSpeechStart = 0
let interruptSilenceStart = 0
let interruptRecorder: MediaRecorder | null = null
let interruptChunks: Blob[] = []

async function maybeInterrupt() {
  if (!interruptEnabled.value || !bgAnalyser || !ttsSource || playingId.value === null) return
  if (interruptRecording) return

  const data = new Uint8Array(bgAnalyser.frequencyBinCount)
  bgAnalyser.getByteTimeDomainData(data)
  let sum = 0
  for (let i = 0; i < data.length; i++) { const v = (data[i] - 128) / 128; sum += v * v }
  const rms = Math.sqrt(sum / data.length)

  const threshold = interruptSensitivity.value
  const now = Date.now()

  if (rms > threshold) {
    // Voice detected
    if (!interruptSpeechStart) interruptSpeechStart = now
    interruptSilenceStart = 0
    const speechMs = now - interruptSpeechStart
    if (speechMs >= interruptSpeechMs.value) {
      // Sustained voice — interrupt!
      stopTTS()
      interruptSpeechStart = 0
      // Start recording for ASR
      if (backgroundStream) {
        interruptRecorder = new MediaRecorder(backgroundStream, { mimeType: 'audio/webm' })
        interruptChunks = []
        interruptRecorder.ondataavailable = (e) => { if (e.data.size > 0) interruptChunks.push(e.data) }
        interruptRecorder.onstop = async () => {
          if (interruptChunks.length === 0) return
          const blob = new Blob(interruptChunks, { type: 'audio/webm' })
          const pcm = await blobToPCM(blob)
          if (pcm.length === 0) return
          try {
            const text = await invoke<string>('transcribe_audio', { audio: Array.from(pcm) })
            if (!text) return
            store.setSending(true)
            store.addMessage({ role: 'user', content: text })
            try {
              const reply = await invoke<string>('chat', { message: text })
              store.addMessage({ role: 'assistant', content: reply })
            } catch (err: any) { store.addMessage({ role: 'assistant', content: String(err) }) }
            finally { store.setSending(false) }
          } catch (err: any) { showToast('Interrupt ASR: ' + String(err)); console.error('ASR error:', err) }
          interruptRecording = false; interruptSpeechStart = 0; interruptSilenceStart = 0
        }
        interruptRecorder.start()
        interruptRecording = true
      }
    }
  } else {
    // Silence
    if (interruptRecording) {
      if (!interruptSilenceStart) interruptSilenceStart = now
      const silenceMs = now - interruptSilenceStart
      if (silenceMs >= interruptSilenceMs.value) {
        interruptRecorder?.stop()
        interruptSilenceStart = 0
      }
    } else {
      interruptSpeechStart = 0
    }
  }
}

// Drive interrupt check from main VAD loop
const interruptTimer = setInterval(maybeInterrupt, 100)

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

onBeforeUnmount(() => {
  clearInterval(interruptTimer)
  ;(interruptRecorder as any)?.stop()
})

async function blobToPCM(blob: Blob): Promise<Float32Array> {
  const ctx = new OfflineAudioContext(1, 1, 16000)
  const audioBuf = await ctx.decodeAudioData(await blob.arrayBuffer())
  return audioBuf.getChannelData(0)
}
// ── Browser ──
const browseUrl = ref('')
const browseResult = ref('')
const browsing = ref(false)

async function browseScreenshot() {
  const url = browseUrl.value.trim()
  if (!url || browsing.value) return
  const target = url.startsWith('http') ? url : `https://${url}`
  browsing.value = true; browseResult.value = ''
  try {
    const b64 = await invoke<string>('browse_screenshot', { url: target })
    browseResult.value = b64
  } catch (err: any) {
    store.addMessage({ role: 'assistant', content: '🌐 ' + String(err) })
  } finally { browsing.value = false }
}
</script>

<template>
  <div class="flex flex-col h-full min-h-0">
    <!-- Toast notification -->
    <div v-if="toast" class="absolute top-4 left-1/2 -translate-x-1/2 z-50 px-4 py-2 bg-red-500 text-white text-sm rounded-lg shadow-lg transition-all duration-300">
      ⚠️ {{ toast }}
    </div>
    <!-- Header -->
    <div class="flex items-center justify-between px-5 h-14 border-b border-gray-100 shrink-0">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-emerald-400" />
        <span class="text-sm font-medium text-gray-700">Companion</span>
        <!-- VAD level bar -->
        <div v-if="recording || vadLevel > 0.05" class="flex items-center gap-1 ml-2">
          <div class="w-16 h-1.5 bg-gray-100 rounded-full overflow-hidden">
            <div class="h-full rounded-full transition-all duration-100"
              :class="vadLevel > 0.4 ? 'bg-red-400' : vadLevel > 0.2 ? 'bg-amber-400' : 'bg-emerald-400'"
              :style="{ width: (vadLevel * 100) + '%' }" />
          </div>
          <span class="text-[10px] text-gray-400 font-mono w-8">{{ (vadLevel * 100).toFixed(0) }}%</span>
        </div>
        <!-- Interrupt status -->
        <span v-if="interruptEnabled && playingId !== null"
          class="text-[10px] text-purple-500 font-mono">⏏ Active</span>
      </div>
      <div class="flex items-center gap-3">
        <div class="flex items-center gap-1.5 text-[11px] text-gray-400">
          <span>{{ voiceMode === 'auto' ? 'Auto' : 'PTT' }}:</span>
          <button @click="startHotkeyCapture"
            class="px-1.5 py-0.5 rounded border border-gray-200 font-mono text-[10px] hover:border-blue-300 transition-colors">
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
        <div v-if="messages.length === 0 && !browseResult" class="flex flex-col items-center justify-center py-20 text-gray-400">
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

    <!-- Bottom bar -->
    <div class="border-t border-gray-100 px-4 py-3 shrink-0">
      <div class="max-w-[720px] mx-auto space-y-2">
        <!-- Browser bar -->
        <div class="flex items-center gap-2">
          <input v-model="browseUrl" type="text" placeholder="https://example.com"
            class="flex-1 bg-gray-50 border border-gray-200 rounded-lg px-3 py-1.5 text-xs outline-none focus:border-blue-300 transition-colors"
            @keydown.enter="browseScreenshot" />
          <button @click="browseScreenshot" :disabled="browsing"
            class="shrink-0 text-xs px-3 py-1.5 rounded-lg bg-gray-100 hover:bg-gray-200 disabled:opacity-30 transition-colors">
            {{ browsing ? '...' : '🌐 Screenshot' }}
          </button>
        </div>
        <!-- Screenshot result -->
        <div v-if="browseResult" class="relative">
          <button @click="browseResult = ''" class="absolute top-1 right-1 text-xs bg-white/80 rounded px-1.5 py-0.5 z-10">✕</button>
          <img :src="browseResult" class="w-full rounded-lg border border-gray-200" />
        </div>
        <!-- Controls -->
        <div class="flex items-center gap-3 flex-wrap">
          <button @click="interruptEnabled = !interruptEnabled"
            :class="['text-[11px] px-2 py-0.5 rounded-full border transition-colors', interruptEnabled ? 'bg-purple-50 border-purple-200 text-purple-600' : 'text-gray-400 border-gray-200 hover:border-gray-300']">
            {{ interruptEnabled ? '⏏ Interrupt ON' : '⏏ Interrupt OFF' }}
          </button>
          <select v-model="ttsVoice"
            class="text-[11px] px-2 py-0.5 rounded-full border border-gray-200 bg-transparent text-gray-500 outline-none">
            <option v-for="v in voices" :key="v" :value="v">{{ v }}</option>
          </select>
          <div class="flex items-center gap-1">
            <span class="text-[11px] text-gray-400">Speed:</span>
            <input v-model.number="ttsSpeed" type="range" min="0.5" max="2.0" step="0.1" class="w-16 h-1 accent-blue-500" />
            <span class="text-[11px] text-gray-400 w-6">{{ ttsSpeed.toFixed(1) }}</span>
          </div>
        </div>
        <div class="flex items-center gap-2 bg-gray-50 border border-gray-200 rounded-2xl px-4 py-1 focus-within:border-blue-300 focus-within:ring-2 focus-within:ring-blue-100 transition-all">
          <button @click="toggleRecord"
            :class="['shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm transition-colors relative', recording ? 'bg-red-100 text-red-500' : 'text-gray-400 hover:text-gray-600 hover:bg-gray-100']"
            :title="recording ? 'Stop' : 'Record'">
            <span v-if="recording" class="text-xs">⏹</span>
            <span v-else>🎤</span>
          </button>
          <input v-model="input" type="text" :placeholder="voiceMode === 'auto' ? 'Speak or type...' : 'Message or ' + hotkey + ' to speak...'"
            class="flex-1 bg-transparent py-2.5 text-[15px] outline-none placeholder:text-gray-400 text-gray-800"
            @keydown.enter="send" :disabled="store.sending" />
          <button @click="send" :disabled="store.sending || !input.trim()"
            class="shrink-0 rounded-xl bg-blue-500 hover:bg-blue-600 disabled:opacity-30 disabled:hover:bg-blue-500 text-white px-4 py-1.5 text-sm font-medium transition-colors">Send</button>
        </div>
      </div>
    </div>
  </div>
</template>
