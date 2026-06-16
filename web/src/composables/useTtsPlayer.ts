/**
 * useTtsPlayer — TTS playback with lip sync.
 *
 * Extracted from ChatView.vue to keep the view lean.
 */
import { ref } from 'vue'
import type { Ref } from 'vue'
import type { ChatMessage } from '../stores/app'

export function useTtsPlayer(opts: {
  ttsVoice: Ref<string>
  ttsSpeed: Ref<number>
  voiceState: Ref<'idle' | 'listening' | 'processing' | 'speaking'>
  synthesizeAudio: (text: string, voice?: string) => Promise<number[]>
  setLipLevel: (level: number) => Promise<void>
}) {
  const playingId = ref<number | null>(null)
  let audioCtx: AudioContext | null = null
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

  function stopTTS() {
    if (ttsSource) { try { ttsSource.stop() } catch {}; ttsSource = null }
    if (audioCtx) { try { audioCtx.close() } catch {}; audioCtx = null }
    playingId.value = null
    if (opts.voiceState.value === 'speaking') opts.voiceState.value = 'idle'
  }

  async function playTTS(text: string, msgIdx: number) {
    if (playingId.value === msgIdx) { stopTTS(); return }
    stopTTS()
    playingId.value = msgIdx
    opts.voiceState.value = 'speaking'

    const chunks = chunkForTTS(text)
    if (chunks.length === 0) { playingId.value = null; return }

    try {
      let pcm = await opts.synthesizeAudio(chunks[0], opts.ttsVoice.value)
      if (!pcm || pcm.length === 0) { playingId.value = null; return }

      let nextPcm: number[] | null = null
      if (chunks.length > 1) {
        opts.synthesizeAudio(chunks[1], opts.ttsVoice.value)
          .then(p => { nextPcm = p }).catch(() => {})
      }

      for (let ci = 0; ci < chunks.length; ci++) {
        if (ci > 0) {
          const startWait = Date.now()
          while (!nextPcm && (Date.now() - startWait) < 15000) {
            await new Promise(r => setTimeout(r, 50))
          }
          pcm = nextPcm!
          if (!pcm || pcm.length === 0) break
          nextPcm = null
          if (ci + 1 < chunks.length) {
            opts.synthesizeAudio(chunks[ci + 1], opts.ttsVoice.value)
              .then(p => { nextPcm = p }).catch(() => {})
          }
        }

        if (playingId.value !== msgIdx) return

        if (!audioCtx) audioCtx = new AudioContext()
        const buf = audioCtx.createBuffer(1, pcm!.length, 16000)
        const ch = buf.getChannelData(0)
        for (let i = 0; i < pcm!.length; i++) ch[i] = pcm![i]
        ttsSource = audioCtx.createBufferSource()
        ttsSource.buffer = buf
        ttsSource.playbackRate.value = opts.ttsSpeed.value
        ttsSource.connect(audioCtx.destination)

        // VU volumes for lip sync
        const chunkSamples = 320
        const volumes: number[] = []
        for (let i = 0; i < pcm!.length; i += chunkSamples) {
          let sum = 0, n = 0
          for (let j = i; j < Math.min(i + chunkSamples, pcm!.length); j++, n++) sum += pcm![j] * pcm![j]
          volumes.push(Math.sqrt(sum / (n || 1)))
        }
        const maxV = Math.max(...volumes, 0.001)
        for (let i = 0; i < volumes.length; i++) volumes[i] = Math.min(volumes[i] / maxV * 2.5, 1)

        const startTime = audioCtx.currentTime
        let lipFrame = 0
        const lipIv = setInterval(() => {
          if (!audioCtx) { clearInterval(lipIv); return }
          const idx = Math.floor((audioCtx.currentTime - startTime) * 1000 / 20)
          if (idx >= volumes.length) { clearInterval(lipIv); opts.setLipLevel(0).catch(() => {}); return }
          if (idx !== lipFrame) { lipFrame = idx; opts.setLipLevel(volumes[idx]).catch(() => {}) }
        }, 20)

        await new Promise<void>(resolve => { ttsSource!.onended = () => resolve(); ttsSource!.start() })
        clearInterval(lipIv)
        opts.setLipLevel(0).catch(() => {})
      }

      playingId.value = null
      ttsSource = null
      if (opts.voiceState.value === 'speaking') opts.voiceState.value = 'idle'
    } catch (err: any) {
      console.error('TTS error:', err)
      playingId.value = null
      opts.voiceState.value = 'idle'
    }
  }

  // Auto-TTS is triggered by caller after doChatSend completes.
  // Removed the message-length watch — it fired on empty bubbles and
  // never re-fired when streaming filled the content (content change ≠ length change).

  return { playingId, playTTS, stopTTS, chunkForTTS }
}
