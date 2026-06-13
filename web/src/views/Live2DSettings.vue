<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const avatarVisible = ref(true)
const alwaysOnTop = ref(true)
const selectedModel = ref('haru')
const modelList = ref<string[]>([])

onMounted(async () => {
  try { avatarVisible.value = await invoke<boolean>('get_avatar_visible') } catch {}
  try {
    const cfg = await invoke<{ live2d_model: string }>('get_config')
    if (cfg.live2d_model) selectedModel.value = cfg.live2d_model
  } catch {}
  try {
    modelList.value = await invoke<string[]>('list_live2d_models')
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

function switchModel(path: string) {
  selectedModel.value = path
  // Save to config
  invoke<any>('get_config').then(cfg => {
    cfg.live2d_model = path
    invoke('update_config', { newConfig: cfg }).catch(() => {})
  }).catch(() => {})
  // Tell avatar window to reload
  invoke('set_live2d_model', { modelPath: path }).catch(() => {})
}

function resetAvatar() {
  invoke('reset_avatar_position').catch(() => {})
}

function modelLabel(path: string) {
  // Extract meaningful name from path like "Epsilon/runtime/Epsilon.model3.json" → "Epsilon"
  // or "mao_pro_zh/mao_pro_zh/runtime/mao_pro.model3.json" → "Mao Pro"
  const parts = path.split('/')
  // Use the first meaningful non-runtime directory name
  const dir = parts[0] || ''
  // Clean up: remove _zh/_jp/_en suffixes, replace underscores with spaces, capitalize
  return dir
    .replace(/_zh$/, '').replace(/_jp$/, '').replace(/_en$/, '').replace(/_pro$/, ' Pro')
    .replace(/_/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase())
    .trim()
}
</script>

<template>
  <div class="flex-1 p-6 overflow-y-auto max-w-2xl">
    <h1 class="text-lg font-semibold text-gray-900 mb-6">Live2D Settings</h1>

    <div class="space-y-6">
      <!-- Model selection -->
      <div class="bg-white rounded-xl border border-gray-200 p-5">
        <h2 class="text-sm font-semibold text-gray-900 mb-3">Character Model</h2>
        <select v-model="selectedModel" @change="switchModel(selectedModel)"
          class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none">
          <option v-for="m in modelList" :key="m" :value="m">{{ modelLabel(m) }}</option>
        </select>
        <p class="text-xs text-gray-400 mt-2">{{ modelList.length }} models found</p>
      </div>

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
