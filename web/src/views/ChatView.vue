<script setup lang="ts">
import { ref, nextTick, watch, onMounted, onBeforeUnmount } from 'vue'
import { useCompanion } from '../composables/useCompanion'
import { useAppStore } from '../stores/app'
import MarkdownRenderer from '../components/MarkdownRenderer.vue'

const { getConfig, updateConfig, chat, chatStream, transcribeAudio, synthesizeAudio, setLipLevel, browseScreenshot } = useCompanion()

/** Strip markdown + nested delimiters for TTS playback.
 *  Brackets [...], parentheses (...), and asterisks *...* are removed
 *  with proper depth counting (won't mismatched-nest). */
function stripForTTS(text: string): string {
  // Step 1: Remove nested delimiters with depth counting
  let out = "";
  let bracketDepth = 0, parenDepth = 0, starDepth = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === '[') bracketDepth++;
    else if (ch === ']' && bracketDepth > 0) { bracketDepth--; continue; }
    else if (ch === '(') parenDepth++;
    else if (ch === ')' && parenDepth > 0) { parenDepth--; continue; }
    else if (ch === '*') { starDepth ^= 1; continue; } // toggle
    else if (bracketDepth > 0 || parenDepth > 0 || starDepth > 0) continue;
    out += ch;
  }
  // Step 2: Strip remaining markdown
  return out
    .replace(/```[\s\S]*?```/g, '')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/[#_~]{1,3}/g, '')
    .replace(/!?\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/^>\s?/gm, '')
    .replace(/^[-*+]\s/gm, '')
    .replace(/^\d+\.\s/gm, '')
    .replace(/---+/g, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

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

// ── Voice input state machine ──
// Replaces recording/interruptRecording/voiceInFlight with single source of truth
type VoiceState = 'idle' | 'listening' | 'processing' | 'speaking'
const voiceState = ref<VoiceState>('idle')
function tryTransition(to: VoiceState): boolean {
  const from = voiceState.value
  const ok = 
    (from === 'idle' && (to === 'listening' || to === 'speaking')) ||
    (from === 'listening' && (to === 'processing' || to === 'idle')) ||
    (from === 'processing' && (to === 'idle' || to === 'speaking')) ||
    (from === 'speaking' && (to === 'idle' || to === 'listening'))
  if (ok) voiceState.value = to
  return ok
}
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
  if (!tryTransition('listening')) return
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
      if (!analyserNode || voiceState.value !== 'listening') { autoVadRaf = 0; return }
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

      if (voiceState.value === 'listening') autoVadRaf = requestAnimationFrame(vadLoop)
    }
    autoVadRaf = requestAnimationFrame(vadLoop)

    mediaRecorder = new MediaRecorder(stream, { mimeType: 'audio/webm' })
    audioChunks = []
    mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) audioChunks.push(e.data) }
    mediaRecorder.onstop = async () => {
      analyserNode = null
      vadLevel.value = 0
      if (audioChunks.length === 0) { voiceState.value = 'idle'; return }
      // Transition to processing
      voiceState.value = 'processing'
      const blob = new Blob(audioChunks, { type: 'audio/webm' })
      const pcm = await blobToPCM(blob)
      if (pcm.length === 0) { voiceState.value = 'idle'; return }
      try {
        const text = await transcribeAudio(Array.from(pcm))
        if (!text) { voiceState.value = 'idle'; return }
        store.setSending(true)
        store.addMessage({ role: 'user', content: text })
        try {
          const reply = await chat(text)
          store.addMessage({ role: 'assistant', content: reply })
        } catch (err: any) { store.addMessage({ role: 'assistant', content: String(err) }) }
        finally { store.setSending(false) }
      } catch (err: any) { showToast('ASR: ' + String(err)); console.error('ASR error:', err) }
      voiceState.value = 'idle'
    }
    mediaRecorder.start()
    // recording.value already set by tryTransition in startRecording
  } catch (err: any) { showToast('Mic: ' + String(err)); console.error('Mic error:', err) }
}

function stopRecording() {
  if (autoVadRaf) { cancelAnimationFrame(autoVadRaf); autoVadRaf = 0 }
  if (mediaRecorder && mediaRecorder.state === 'recording') mediaRecorder.stop()
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
  // Load voice preferences from config
  getConfig().then(c => {
    if (c.voice_mode === 'auto' || c.voice_mode === 'ptt') voiceMode.value = c.voice_mode
    if (c.tts_voice) ttsVoice.value = c.tts_voice
    if (c.tts_speed) ttsSpeed.value = c.tts_speed
    if (c.tts_auto_play !== undefined) ttsAuto.value = c.tts_auto_play
  }).catch(() => {})
})

onBeforeUnmount(() => {
  cancelAnimationFrame(bgVadLoop)
  bgAnalyser = null
  bgAudioCtx?.close()
  backgroundStream?.getTracks().forEach(t => t.stop())
})

function toggleRecord() { voiceState.value === 'listening' ? stopRecording() : startRecording() }


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
let ttsLastAutoMs = 0
let ttsSource: AudioBufferSourceNode | null = null

/** Split text into speakable chunks (sentence boundaries, max ~200 chars each). */
function chunkForTTS(text: string): string[] {
  const chunks: string[] = []
  const sentences = text.split(/(?<=[.。！？!?\n])\s*/)
  let buf = ''
  for (const s of sentences) {
    if (!s.trim()) continue
    if (buf.length + s.length > 200 && buf.length > 50) {
      chunks.push(buf.trim())
      buf = s
    } else {
      buf += s
    }
  }
  if (buf.trim()) chunks.push(buf.trim())
  return chunks.length > 0 ? chunks : [text.slice(0, 200)]
}

async function playTTS(text: string, msgIdx: number) {
  if (playingId.value === msgIdx) { stopTTS(); return }
  stopTTS()
  playingId.value = msgIdx
  voiceState.value = 'speaking'

  const chunks = chunkForTTS(text)
  if (chunks.length === 0) { playingId.value = null; return }

  try {
    // Synthesize first chunk immediately
    let pcm = await synthesizeAudio(chunks[0], ttsVoice.value)
    if (!pcm || pcm.length === 0) { playingId.value = null; return }

    // Start pre-fetching second chunk while playing first
    let nextPcm: number[] | null = null
    if (chunks.length > 1) {
      synthesizeAudio(chunks[1], ttsVoice.value)
        .then(p => { nextPcm = p }).catch(() => {})
    }

    for (let ci = 0; ci < chunks.length; ci++) {
      // If not first chunk, wait for the pre-fetched PCM
      if (ci > 0) {
        const MAX_WAIT = 15000
        const startWait = Date.now()
        while (!nextPcm && (Date.now() - startWait) < MAX_WAIT) {
          await new Promise(r => setTimeout(r, 50))
        }
        pcm = nextPcm!
        if (!pcm || pcm.length === 0) break
        // Pre-fetch next
        nextPcm = null
        if (ci + 1 < chunks.length) {
          synthesizeAudio(chunks[ci + 1], ttsVoice.value)
            .then(p => { nextPcm = p }).catch(() => {})
        }
      }

      // If user stopped or switched to another message, abort
      if (playingId.value !== msgIdx) return

      if (!audioCtx) audioCtx = new AudioContext()
      const buf = audioCtx.createBuffer(1, pcm!.length, 16000)
      const ch = buf.getChannelData(0)
      for (let i = 0; i < pcm!.length; i++) ch[i] = pcm![i]
      ttsSource = audioCtx.createBufferSource()
      ttsSource.buffer = buf
      ttsSource.playbackRate.value = ttsSpeed.value
      ttsSource.connect(audioCtx.destination)

      // Precompute VU volumes: one RMS value per 20ms chunk (= 320 samples @ 16kHz)
      const chunkSamples = 320;
      const volumes: number[] = [];
      for (let i = 0; i < pcm!.length; i += chunkSamples) {
        let sum = 0, n = 0;
        for (let j = i; j < Math.min(i + chunkSamples, pcm!.length); j++, n++) sum += pcm![j] * pcm![j];
        volumes.push(Math.sqrt(sum / (n || 1)));
      }
      // Normalize to 0–1 range
      const maxV = Math.max(...volumes, 0.001);
      for (let i = 0; i < volumes.length; i++) volumes[i] = Math.min(volumes[i] / maxV * 2.5, 1);

      // Lip sync: read next volume every 20ms matching audio playback
      const startTime = audioCtx.currentTime;
      let lipFrame = 0;
      const lipIv = setInterval(() => {
        if (!audioCtx) { clearInterval(lipIv); return; }
        const idx = Math.floor((audioCtx.currentTime - startTime) * 1000 / 20);
        if (idx >= volumes.length) { clearInterval(lipIv); setLipLevel(0).catch(() => {}); return; }
        if (idx !== lipFrame) { lipFrame = idx; setLipLevel(volumes[idx]).catch(() => {}); }
      }, 20);

      // Wait for playback to finish
      await new Promise<void>(resolve => { ttsSource!.onended = () => resolve(); ttsSource!.start() })

      // Cleanup
      clearInterval(lipIv)
      setLipLevel(0).catch(() => {})
    }

    playingId.value = null
    ttsSource = null
    if (voiceState.value === 'speaking') voiceState.value = 'idle'
  } catch (err: any) {
    showToast('TTS: ' + String(err))
    console.error('TTS error:', err)
    playingId.value = null
    voiceState.value = 'idle'
  }
}

function stopTTS() {
  if (ttsSource) { try { ttsSource.stop() } catch {}; ttsSource = null }
  if (audioCtx) { try { audioCtx.close() } catch {}; audioCtx = null }
  playingId.value = null
  if (voiceState.value === 'speaking') voiceState.value = 'idle'
}

// ── Streaming state ──
const streamingIdx = ref(-1) // message index currently being streamed

// Listen for streaming tokens
import('@tauri-apps/api/event').then(m => {
  m.listen<{ token?: string; done?: boolean }>('chat_token', (evt) => {
    const p = evt.payload
    if (p.done) { streamingIdx.value = -1; return }
    if (p.token && streamingIdx.value >= 0) {
      store.messages[streamingIdx.value].content += p.token
    }
  })
  // Alt+` global hotkey: insert ASR result into chat input
  m.listen<{ text: string }>('voice_asr_result', (evt) => {
    input.value = evt.payload.text
  })
}).catch(() => {})

// ── Chat ──
async function send() {
  const text = input.value.trim()
  if (!text || store.sending) return
  input.value = ''; store.setSending(true)
  store.addMessage({ role: 'user', content: text })
  
  // Add empty assistant bubble for streaming
  store.addMessage({ role: 'assistant', content: '' })
  streamingIdx.value = store.messages.length - 1
  
  try {
    await chatStream(text) // streaming — tokens arrive via chat_token events
    store.setSending(false)
    streamingIdx.value = -1
  } catch (err: any) {
    if (streamingIdx.value >= 0) {
      store.messages[streamingIdx.value].content = String(err)
    } else {
      store.addMessage({ role: 'assistant', content: String(err) })
    }
    store.setSending(false)
    streamingIdx.value = -1
  }
}

// ── Interrupt: background VAD with dual threshold ──
const interruptSpeechMs = ref(300)     // ms of sustained voice to trigger
const interruptSilenceMs = ref(800)    // ms of silence after speech to stop recording
let interruptRecording = false
let voiceInFlight = false // shared lock — prevent PTT + interrupt overlap
let interruptSpeechStart = 0
let interruptSilenceStart = 0
let interruptRecorder: MediaRecorder | null = null
let interruptChunks: Blob[] = []

async function maybeInterrupt() {
  if (!interruptEnabled.value || !bgAnalyser || !ttsSource || playingId.value === null) return
  // Only trigger interrupt from 'speaking' state
  if (voiceState.value !== 'speaking') return

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
      if (backgroundStream && tryTransition('listening')) {
        // Kill PTT recorder so it doesn't keep firing onstop
        if (autoVadRaf) { cancelAnimationFrame(autoVadRaf); autoVadRaf = 0 }
        if (mediaRecorder && mediaRecorder.state === 'recording') {
          mediaRecorder.ondataavailable = null  // prevent stale data
          mediaRecorder.onstop = null           // prevent stale onstop
          mediaRecorder.stop()
        }
        mediaRecorder = null
        interruptRecorder = new MediaRecorder(backgroundStream, { mimeType: 'audio/webm' })
        interruptChunks = []
        interruptRecorder.ondataavailable = (e) => { if (e.data.size > 0) interruptChunks.push(e.data) }
        interruptRecorder.onstop = async () => {
          if (interruptChunks.length === 0) { voiceState.value = 'idle'; return }
          voiceState.value = 'processing'
          const blob = new Blob(interruptChunks, { type: 'audio/webm' })
          const pcm = await blobToPCM(blob)
          if (pcm.length === 0) { voiceState.value = 'idle'; return }
          try {
            const text = await transcribeAudio(Array.from(pcm))
            if (!text) { voiceState.value = 'idle'; return }
            store.setSending(true)
            store.addMessage({ role: 'user', content: text })
            try {
              const reply = await chat(text)
              store.addMessage({ role: 'assistant', content: reply })
            } catch (err: any) { store.addMessage({ role: 'assistant', content: String(err) }) }
            finally { store.setSending(false) }
          } catch (err: any) { showToast('Interrupt ASR: ' + String(err)); console.error('ASR error:', err) }
          voiceState.value = 'idle'
          interruptSpeechStart = 0; interruptSilenceStart = 0
        }
        interruptRecorder.start()
      }
    }
  } else {
    // Silence — check if interrupt recorder is still running
    if (interruptRecorder && interruptRecorder.state === 'recording') {
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
      playTTS(stripForTTS(last.content), len - 1).catch(e => console.error('[TTS] watch playTTS error:', e))
    }
  }
})

onBeforeUnmount(() => {
  clearInterval(interruptTimer)
  ;(interruptRecorder as any)?.stop()
})

// Persist TTS voice + speed changes to config
watch(ttsVoice, async (val) => {
  try {
    const c = await getConfig()
    c.tts_voice = val
    await updateConfig(c)
  } catch {}
})
watch(ttsSpeed, async (val) => {
  try {
    const c = await getConfig()
    c.tts_speed = val
    await updateConfig(c)
  } catch {}
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

async function doBrowseScreenshot() {
  const url = browseUrl.value.trim()
  if (!url || browsing.value) return
  const target = url.startsWith('http') ? url : `https://${url}`
  browsing.value = true; browseResult.value = ''
  try {
    const b64 = await browseScreenshot(target)
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
        <div v-if="voiceState === 'listening' || vadLevel > 0.05" class="flex items-center gap-1 ml-2">
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
              <!-- Assistant message: render markdown -->
              <div class="max-w-[75%] rounded-2xl rounded-bl-md bg-gray-50 border border-gray-100 px-4 py-2.5 text-gray-800">
                <MarkdownRenderer :content="m.content" />
              </div>
              <button @click="playTTS(stripForTTS(m.content), i)"
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
            @keydown.enter="doBrowseScreenshot" />
          <button @click="doBrowseScreenshot" :disabled="browsing"
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
            :class="['shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm transition-colors relative', voiceState === 'listening' ? 'bg-red-100 text-red-500' : 'text-gray-400 hover:text-gray-600 hover:bg-gray-100']"
            :title="voiceState === 'listening' ? 'Stop' : 'Record'">
            <span v-if="voiceState === 'listening'" class="text-xs">⏹</span>
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
