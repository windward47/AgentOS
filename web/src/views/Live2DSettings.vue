<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const avatarVisible = ref(true)
const alwaysOnTop = ref(true)
const avatarScale = ref(0.12)

onMounted(async () => {
  try {
    const visible = await invoke<boolean>('get_avatar_visible')
    avatarVisible.value = visible
  } catch {}
})

function toggleAvatar() {
  avatarVisible.value = !avatarVisible.value
  invoke('set_avatar_visible', { visible: avatarVisible.value }).catch(() => {})
}

function toggleAlwaysOnTop() {
  alwaysOnTop.value = !alwaysOnTop.value
  invoke('set_avatar_always_on_top', { onTop: alwaysOnTop.value }).catch(() => {})
}

function resetAvatar() {
  invoke('reset_avatar_position').catch(() => {})
}
</script>

<template>
  <div class="flex-1 p-6 overflow-y-auto max-w-2xl">
    <h1 class="text-lg font-semibold text-gray-900 mb-6">Live2D Settings</h1>

    <div class="space-y-6">
      <!-- Avatar toggle -->
      <div class="bg-white rounded-xl border border-gray-200 p-5">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-sm font-semibold text-gray-900">Avatar Window</h2>
            <p class="text-xs text-gray-500 mt-0.5">Show or hide the Live2D character</p>
          </div>
          <button @click="toggleAvatar"
            :class="['relative w-11 h-6 rounded-full transition-colors', avatarVisible ? 'bg-blue-600' : 'bg-gray-300']">
            <span :class="['absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform', avatarVisible ? 'translate-x-5' : '']" />
          </button>
        </div>
      </div>

      <!-- Always on top -->
      <div class="bg-white rounded-xl border border-gray-200 p-5">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-sm font-semibold text-gray-900">Always on Top</h2>
            <p class="text-xs text-gray-500 mt-0.5">Keep the avatar window above other windows</p>
          </div>
          <button @click="toggleAlwaysOnTop"
            :class="['relative w-11 h-6 rounded-full transition-colors', alwaysOnTop ? 'bg-blue-600' : 'bg-gray-300']">
            <span :class="['absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform', alwaysOnTop ? 'translate-x-5' : '']" />
          </button>
        </div>
      </div>

      <!-- Reset position -->
      <div class="bg-white rounded-xl border border-gray-200 p-5">
        <h2 class="text-sm font-semibold text-gray-900 mb-1">Reset Position</h2>
        <p class="text-xs text-gray-500 mb-3">Move the avatar window and model back to default position</p>
        <button @click="resetAvatar"
          class="px-4 py-1.5 text-sm rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 transition-colors">
          Reset
        </button>
      </div>
    </div>
  </div>
</template>
