<script setup lang="ts">
/**
 * AvatarWindow — dedicated transparent overlay window for the Live2D character.
 *
 * The actual rendering happens in the avatar window (web/public/avatar.html),
 * which loads pixi-live2d-display + Cubism Core from CDN.
 * See `web/src/avatar/main.ts` for the render loop and interaction handling.
 *
 * This Vue component is only used as the Tauri window entry point
 * (route: /avatar). No Cubism SDK code lives in the web/src/ tree — it's
 * all in the pre-built avatar-agent bundle.
 */

import { onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'

onMounted(async () => {
  // Transparent background for the avatar overlay window
  document.documentElement.style.background = 'transparent'
  document.body.style.background = 'transparent'
  const root = document.getElementById('app')
  if (root) root.style.background = 'transparent'

  // Listen for audio_level events from main window (for lip sync)
  await listen<{ level: number }>('audio_level', () => {
    // TODO: drive mouth open parameter once Live2D model is loaded
    // renderer.setParam('ParamMouthOpenY', e.payload.level)
  })
})
</script>

<template>
  <div class="flex items-center justify-center w-full h-full">
    <!-- TODO: replace with <canvas ref="canvasRef" /> once Cubism SDK is set up -->
    <div class="text-center opacity-50 select-none">
      <div class="text-6xl mb-3">🎭</div>
      <p class="text-sm text-white/60 tracking-wide">AVATAR</p>
    </div>
  </div>
</template>

<style scoped>
/* Avatar window is fully transparent — just the character is visible */
</style>
