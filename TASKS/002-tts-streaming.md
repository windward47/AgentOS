# 002 — TTS Streaming Chunking

Status: done

## Goal
Fix TTS to play sentence-by-sentence during streaming, not just at end.

## Done
- [x] Simplified useTtsPlayer (removed pre-fetch complexity)
- [x] chunkForTTS splits by 。！？\n boundaries
- [x] playTTS loops chunks, skip on error
- [x] Auto-TTS triggers on done event
- [x] Manual 🔊 button per message
- [x] Auto/manual toggle + voice/speed controls in chat bar
