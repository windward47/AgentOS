/**
 * useTtsPlayer — TTS playback engine.
 * Splits text by sentence boundaries, plays one chunk at a time.
 */
import { ref } from 'vue'
import type { Ref } from 'vue'

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

  /** Split text into speakable chunks at sentence boundaries, max ~200 chars each. */
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

    for (let ci = 0; ci < chunks.length; ci++) {
      if (playingId.value !== msgIdx) break
      let pcm: number[] | null = null
      try { pcm = await opts.synthesizeAudio(chunks[ci], opts.ttsVoice.value) } catch { /* skip */ }
      if (!pcm || pcm.length === 0) continue

      if (!audioCtx) audioCtx = new AudioContext()
      try {
        const buf = audioCtx.createBuffer(1, pcm.length, 16000)
        buf.getChannelData(0).set(pcm)
        ttsSource = audioCtx.createBufferSource()
        ttsSource.buffer = buf
        ttsSource.playbackRate.value = opts.ttsSpeed.value
        ttsSource.connect(audioCtx.destination)
        await new Promise<void>(resolve => { ttsSource!.onended = () => resolve(); ttsSource!.start() })
      } catch { /* skip failed playback */ }
    }

    playingId.value = null
    ttsSource = null
    if (opts.voiceState.value === 'speaking') opts.voiceState.value = 'idle'
  }

  return { playingId, playTTS, stopTTS, chunkForTTS }
}
