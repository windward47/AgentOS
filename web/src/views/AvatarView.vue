<script setup lang="ts">
/**
 * AvatarWindow — dedicated transparent overlay window for the Live2D character.
 *
 * REAL LIVE2D INTEGRATION (requires Cubism 4 SDK for Web):
 *   1. Download Cubism SDK for Web from https://www.live2d.com/download/cubism-sdk/
 *   2. Copy Core/live2dcubismcore.min.js → web/public/live2d/
 *   3. Copy Framework/dist/live2dcubismframework.min.js → web/public/live2d/
 *   4. Place model files in web/public/models/<model-name>/
 *   5. Uncomment the init block below and replace the placeholder values
 *
 * The renderer module (web/src/live2d/renderer.ts) handles the raw Cubism Core
 * API — no pixi.js or third-party library required.
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
