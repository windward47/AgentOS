# 001 — Voice Input Integration

Status: pending

## Goal
Integrate useVoiceCore into ChatView for push-to-talk voice input.

## Steps
- [ ] Add mic button to ChatView input bar
- [ ] Wire toggleRecord to hotkey (Ctrl+Shift+V)
- [ ] ASR result → send(text)
- [ ] Voice state indicator in header

## Dependencies
- useVoiceCore composable (already written)
- ASR provider configured in Settings
