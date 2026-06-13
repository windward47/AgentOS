<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useCompanion } from '../composables/useCompanion'
import { listen } from '@tauri-apps/api/event'

interface ModelEntry {
  id: string
  name: string
  description: string
  size: string
  url: string
}

interface DownloadProgress {
  phase: 'downloading' | 'extracting' | 'done' | 'error'
  downloaded?: number
  total?: number
  model_id?: string
  message?: string
}

const {
  getConfig, updateConfig,
  getAvatarVisible, setAvatarVisible, setAvatarAlwaysOnTop,
  listLive2dModels, setLive2dModel, resetAvatarPosition,
  downloadModel,
} = useCompanion()

const avatarVisible = ref(true)
const alwaysOnTop = ref(true)
const selectedModel = ref('haru')
const modelList = ref<string[]>([])

// Model Store
const storeModels = ref<ModelEntry[]>([])
const downloadingId = ref<string | null>(null)
const downloadPercent = ref(0)
const downloadError = ref('')
let unlisten: (() => void) | null = null

onMounted(async () => {
  try { avatarVisible.value = await getAvatarVisible() } catch {}
  try {
    const cfg = await getConfig()
    if (cfg.live2d_model) selectedModel.value = cfg.live2d_model
  } catch {}
  try {
    modelList.value = await listLive2dModels()
  } catch {}
  try {
    const resp = await fetch('/models-manifest.json')
    storeModels.value = await resp.json()
  } catch {}

  unlisten = await listen<DownloadProgress>('download_progress', (evt) => {
    const p = evt.payload
    if (p.phase === 'downloading' && p.total && p.total > 0) {
      downloadPercent.value = Math.round((p.downloaded! / p.total) * 100)
    } else if (p.phase === 'extracting') {
      downloadPercent.value = 100
    } else if (p.phase === 'done') {
      downloadingId.value = null
      downloadPercent.value = 0
      downloadError.value = ''
      refreshModels()
    } else if (p.phase === 'error') {
      downloadingId.value = null
      downloadPercent.value = 0
      downloadError.value = p.message || 'Download failed'
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

function isDownloaded(id: string): boolean {
  return modelList.value.some(m => m.includes(id.replace(/_pro_/g, '')) || m.startsWith(id))
}

function doDownload(entry: ModelEntry) {
  downloadingId.value = entry.id
  downloadPercent.value = 0
  downloadError.value = ''
  downloadModel(entry.url, entry.id).catch(e => {
    downloadError.value = String(e)
    downloadingId.value = null
  })
}

async function refreshModels() {
  try {
    modelList.value = await listLive2dModels()
  } catch {}
}

function toggleAvatar() {
  avatarVisible.value = !avatarVisible.value
  setAvatarVisible(avatarVisible.value).catch(() => {})
}

function toggleAlwaysOnTop() {
  alwaysOnTop.value = !alwaysOnTop.value
  setAvatarAlwaysOnTop(alwaysOnTop.value).catch(() => {})
}

function switchModel(path: string) {
  selectedModel.value = path
  getConfig().then(cfg => {
    cfg.live2d_model = path
    updateConfig(cfg).catch(() => {})
  }).catch(() => {})
  setLive2dModel(path).catch(() => {})
}

function resetAvatar() {
  resetAvatarPosition().catch(() => {})
}

function modelLabel(path: string) {
  const parts = path.split('/')
  const dir = parts[0] || ''
  const sub = parts.length > 2 ? parts[1] : ''
  const label = sub ? dir + '/' + sub : dir
  return label
    .replace(/_zh$/, '').replace(/_jp$/, '').replace(/_en$/, '').replace(/_pro$/, ' Pro')
    .replace(/_/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase())
    .trim()
}

function storeLabel(entry: ModelEntry): string {
  return entry.name + ' (' + entry.size + ')'
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

      <!-- Model Store -->
      <div class="bg-white rounded-xl border border-gray-200 p-5">
        <h2 class="text-sm font-semibold text-gray-900 mb-3">Model Store</h2>
        <p class="text-xs text-gray-500 mb-4">Download additional Live2D characters from GitHub Releases.</p>

        <div v-if="storeModels.length === 0" class="text-xs text-gray-400 italic">
          Loading model catalog...
        </div>

        <div v-for="entry in storeModels" :key="entry.id"
          class="flex items-center justify-between py-2.5 border-b border-gray-100 last:border-0">
          <div class="flex-1 min-w-0">
            <div class="text-sm text-gray-800 font-medium truncate">{{ entry.name }}</div>
            <div class="text-xs text-gray-400 mt-0.5">{{ entry.description }} · {{ entry.size }}</div>
          </div>

          <div class="ml-3 flex-shrink-0">
            <span v-if="isDownloaded(entry.id)"
              class="inline-flex items-center gap-1 text-xs text-green-600 font-medium">
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
              </svg>
              Installed
            </span>

            <div v-else-if="downloadingId === entry.id" class="flex items-center gap-2">
              <div class="w-16 h-1.5 bg-gray-200 rounded-full overflow-hidden">
                <div class="h-full bg-blue-500 rounded-full transition-all duration-300"
                  :style="{ width: downloadPercent + '%' }" />
              </div>
              <span class="text-xs text-gray-500">{{ downloadPercent }}%</span>
            </div>

            <button v-else @click="doDownload(entry)"
              class="px-3 py-1 text-xs rounded-lg border border-blue-200 text-blue-600 hover:bg-blue-50 transition-colors">
              Download
            </button>
          </div>
        </div>

        <div v-if="downloadError" class="mt-3 text-xs text-red-500 bg-red-50 rounded-lg px-3 py-2">
          {{ downloadError }}
        </div>
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
