/**
 * useVoiceCore — voice recording (PTT) + background VAD + interrupt handling.
 *
 * Extracted from ChatView.vue.
 */
import { ref } from 'vue'
import type { Ref } from 'vue'
import type { ChatMessage } from '../stores/app'

export function useVoiceCore(opts: {
  voiceState: Ref<'idle' | 'listening' | 'processing' | 'speaking'>
  voiceMode: Ref<'ptt' | 'auto'>
  messages: Ref<ChatMessage[]>
  store: { addMessage: (m: ChatMessage) => void; setSending: (v: boolean) => void; sending: boolean }
  transcribeAudio: (audio: number[]) => Promise<string>
  doChatSend: (text: string) => Promise<void>
  showToast: (msg: string) => void
  tryTransition: (to: 'idle' | 'listening' | 'processing' | 'speaking') => boolean
}) {
  // ── Recording state ──
  const vadLevel = ref(0)
  const interruptEnabled = ref(true)
  const interruptSensitivity = ref(0.3)

  let autoVadRaf = 0
  let autoSpeechStart = 0
  let autoSilenceStart = 0
  const AUTO_MIN_SPEECH_MS = 400
  const AUTO_SILENCE_MS = 1300

  let mediaRecorder: MediaRecorder | null = null
  let audioChunks: Blob[] = []
  let analyserNode: AnalyserNode | null = null

  // ── Background mic ──
  let backgroundStream: MediaStream | null = null
  let bgAnalyser: AnalyserNode | null = null
  let bgVadLoop = 0
  let bgAudioCtx: AudioContext | null = null
  let micReady = false

  // ── Interrupt state ──
  let interruptRecorder: MediaRecorder | null = null
  let interruptChunks: Blob[] = []
  let interruptSpeechStart = 0
  let interruptSilenceStart = 0

  async function blobToPCM(blob: Blob): Promise<Float32Array> {
    const ctx = new OfflineAudioContext(1, 1, 16000)
    const audioBuf = await ctx.decodeAudioData(await blob.arrayBuffer())
    return audioBuf.getChannelData(0)
  }

  // ── PTT Recording ──
  async function startRecording() {
    if (!micReady) return
    if (!opts.tryTransition('listening')) return
    try {
      const stream = backgroundStream!
      // Close any previous recording AudioContext
      if (analyserNode) { analyserNode.disconnect(); analyserNode = null }
      const audioCtx = new AudioContext()
      const source = audioCtx.createMediaStreamSource(stream)
      analyserNode = audioCtx.createAnalyser()
      analyserNode.fftSize = 256
      source.connect(analyserNode)

      const vadData = new Uint8Array(analyserNode.frequencyBinCount)
      const isAuto = opts.voiceMode.value === 'auto'
      const threshold = 0.04
      autoSpeechStart = 0
      autoSilenceStart = 0

      const vadLoop = () => {
        if (!analyserNode || opts.voiceState.value !== 'listening') { autoVadRaf = 0; return }
        analyserNode.getByteTimeDomainData(vadData)
        let sum = 0
        for (let i = 0; i < vadData.length; i++) {
          const v = (vadData[i] - 128) / 128; sum += v * v
        }
        const rms = Math.sqrt(sum / vadData.length) * 3
        vadLevel.value = rms

        if (isAuto) {
          const now = Date.now()
          if (rms > threshold) {
            if (!autoSpeechStart) autoSpeechStart = now
            autoSilenceStart = 0
          } else {
            if (autoSpeechStart && (now - autoSpeechStart) >= AUTO_MIN_SPEECH_MS) {
              if (!autoSilenceStart) autoSilenceStart = now
              if ((now - autoSilenceStart) >= AUTO_SILENCE_MS) {
                stopRecording(); return
              }
            } else { autoSpeechStart = 0 }
          }
        }
        if (opts.voiceState.value === 'listening') autoVadRaf = requestAnimationFrame(vadLoop)
      }
      autoVadRaf = requestAnimationFrame(vadLoop)

      mediaRecorder = new MediaRecorder(stream, { mimeType: 'audio/webm' })
      audioChunks = []
      mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) audioChunks.push(e.data) }
      mediaRecorder.onstop = async () => {
        analyserNode = null; vadLevel.value = 0
        if (audioChunks.length === 0) { opts.voiceState.value = 'idle'; return }
        opts.voiceState.value = 'processing'
        const blob = new Blob(audioChunks, { type: 'audio/webm' })
        const pcm = await blobToPCM(blob)
        if (pcm.length === 0) { opts.voiceState.value = 'idle'; return }
        try {
          const text = await opts.transcribeAudio(Array.from(pcm))
          if (!text) { opts.voiceState.value = 'idle'; return }
          opts.doChatSend(text)
        } catch (err: any) { opts.showToast('ASR: ' + String(err)) }
        opts.voiceState.value = 'idle'
      }
      mediaRecorder.start()
    } catch (err: any) { opts.showToast('Mic: ' + String(err)) }
  }

  function stopRecording() {
    if (autoVadRaf) { cancelAnimationFrame(autoVadRaf); autoVadRaf = 0 }
    if (mediaRecorder && mediaRecorder.state === 'recording') mediaRecorder.stop()
    analyserNode = null; vadLevel.value = 0
  }

  function toggleRecord() {
    opts.voiceState.value === 'listening' ? stopRecording() : startRecording()
  }

  // ── Background VAD + Interrupt ──
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
        bgVadLoop = requestAnimationFrame(loop)
      }
      bgVadLoop = requestAnimationFrame(loop)
      micReady = true
    } catch { /* mic denied */ }
  }

  async function maybeInterrupt(ttsPlaying: boolean) {
    if (!interruptEnabled.value || !bgAnalyser || !ttsPlaying) return
    if (opts.voiceState.value !== 'speaking') return

    const data = new Uint8Array(bgAnalyser.frequencyBinCount)
    bgAnalyser.getByteTimeDomainData(data)
    let sum = 0
    for (let i = 0; i < data.length; i++) { const v = (data[i] - 128) / 128; sum += v * v }
    const rms = Math.sqrt(sum / data.length)
    const threshold = interruptSensitivity.value
    const now = Date.now()

    if (rms > threshold) {
      if (!interruptSpeechStart) interruptSpeechStart = now
      interruptSilenceStart = 0
      if ((now - interruptSpeechStart) >= 300) {
        // Kill TTS and start recording
        if (backgroundStream && opts.tryTransition('listening')) {
          if (autoVadRaf) { cancelAnimationFrame(autoVadRaf); autoVadRaf = 0 }
          if (mediaRecorder && mediaRecorder.state === 'recording') {
            mediaRecorder.ondataavailable = null; mediaRecorder.onstop = null
            mediaRecorder.stop()
          }
          mediaRecorder = null
          interruptRecorder = new MediaRecorder(backgroundStream, { mimeType: 'audio/webm' })
          interruptChunks = []
          interruptRecorder.ondataavailable = (e) => { if (e.data.size > 0) interruptChunks.push(e.data) }
          interruptRecorder.onstop = async () => {
            if (interruptChunks.length === 0) { opts.voiceState.value = 'idle'; return }
            opts.voiceState.value = 'processing'
            const blob = new Blob(interruptChunks, { type: 'audio/webm' })
            const pcm = await blobToPCM(blob)
            if (pcm.length === 0) { opts.voiceState.value = 'idle'; return }
            try {
              const text = await opts.transcribeAudio(Array.from(pcm))
              if (!text) { opts.voiceState.value = 'idle'; return }
              opts.doChatSend(text)
            } catch (err: any) { opts.showToast('Interrupt ASR: ' + String(err)) }
            opts.voiceState.value = 'idle'
            interruptSpeechStart = 0; interruptSilenceStart = 0
          }
          interruptRecorder.start()
        }
      }
    } else {
      if (interruptRecorder && interruptRecorder.state === 'recording') {
        if (!interruptSilenceStart) interruptSilenceStart = now
        if ((now - interruptSilenceStart) >= 800) {
          interruptRecorder?.stop(); interruptSilenceStart = 0
        }
      } else { interruptSpeechStart = 0 }
    }
  }

  function cleanup() {
    cancelAnimationFrame(bgVadLoop)
    bgAnalyser = null; bgAudioCtx?.close()
    backgroundStream?.getTracks().forEach(t => t.stop())
    if (interruptRecorder) { try { interruptRecorder.stop() } catch {} }
  }

  return {
    vadLevel, interruptEnabled, interruptSensitivity,
    startRecording, stopRecording, toggleRecord,
    startBackgroundVAD, maybeInterrupt, cleanup,
    blobToPCM,
  }
}
