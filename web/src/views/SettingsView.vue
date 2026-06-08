<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'

interface Config {
  sandbox_path: string
  llm_provider: string
  asr_provider: string
  tts_provider: string
  system_mode: boolean
  enable_accessibility: boolean
  vad_threshold: number
  user_name: string
  style_template: string
  custom_system_prompt: string | null
  emotion_mapping: Record<string, string>
}

const router = useRouter()
const config = ref<Config | null>(null)
const saving = ref(false)
const saved = ref(false)

const providerOptions = ['openai', 'ollama', 'claude']
const asrOptions = ['local', 'cloud']
const ttsOptions = ['local', 'cloud']
const styleOptions = ['professional', 'humorous', 'gentle', 'geek']

onMounted(async () => {
  try {
    config.value = await invoke<Config>('get_config')
  } catch (e) {
    console.error('Failed to load config:', e)
  }
})

async function save() {
  if (!config.value) return
  saving.value = true
  saved.value = false
  try {
    await invoke('update_config', { config: config.value })
    saved.value = true
    setTimeout(() => { saved.value = false }, 2000)
  } catch (e) {
    console.error('Failed to save config:', e)
  } finally {
    saving.value = false
  }
}

function confirmSystemMode() {
  if (config.value?.system_mode) {
    if (!confirm('Switch to System Mode? Tools can access any file path. Proceed?')) {
      config.value!.system_mode = false
    }
  }
}
</script>

<template>
  <div class="p-6 max-w-2xl mx-auto overflow-y-auto h-full">
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-xl font-bold text-gray-800 dark:text-gray-100">Settings</h1>
      <div class="flex gap-2">
        <span v-if="saved" class="text-xs text-green-500 self-center">Saved</span>
        <button @click="router.push('/')" class="text-xs text-gray-400 hover:text-gray-600 px-2">Back to Chat</button>
      </div>
    </div>

    <div v-if="!config" class="text-gray-400 text-center py-12">Loading...</div>

    <div v-else class="space-y-6">
      <!-- User -->
      <section class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
        <h2 class="text-sm font-semibold text-gray-500 uppercase mb-3">Profile</h2>
        <div class="space-y-3">
          <div>
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">User Name</label>
            <input v-model="config.user_name" type="text" class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">Style Template</label>
            <select v-model="config.style_template" class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm">
              <option v-for="s in styleOptions" :key="s" :value="s">{{ s }}</option>
            </select>
          </div>
        </div>
      </section>

      <!-- Providers -->
      <section class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
        <h2 class="text-sm font-semibold text-gray-500 uppercase mb-3">Providers</h2>
        <div class="grid grid-cols-3 gap-4">
          <div>
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">LLM</label>
            <select v-model="config.llm_provider" class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm">
              <option v-for="p in providerOptions" :key="p" :value="p">{{ p }}</option>
            </select>
          </div>
          <div>
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">ASR</label>
            <select v-model="config.asr_provider" class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm">
              <option v-for="p in asrOptions" :key="p" :value="p">{{ p }}</option>
            </select>
          </div>
          <div>
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">TTS</label>
            <select v-model="config.tts_provider" class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm">
              <option v-for="p in ttsOptions" :key="p" :value="p">{{ p }}</option>
            </select>
          </div>
        </div>
      </section>

      <!-- Sandbox & Security -->
      <section class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
        <h2 class="text-sm font-semibold text-gray-500 uppercase mb-3">Sandbox & Security</h2>
        <div class="space-y-3">
          <div>
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">Sandbox Path</label>
            <input v-model="config.sandbox_path" type="text" class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm font-mono" />
          </div>
          <div class="flex items-center justify-between">
            <span class="text-sm text-gray-600 dark:text-gray-400">System Mode</span>
            <label class="relative inline-flex items-center cursor-pointer">
              <input v-model="config.system_mode" type="checkbox" class="sr-only peer" @click="confirmSystemMode" />
              <div class="w-9 h-5 bg-gray-300 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-orange-500"></div>
            </label>
          </div>
        </div>
      </section>

      <!-- VAD -->
      <section class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
        <h2 class="text-sm font-semibold text-gray-500 uppercase mb-3">Voice Activity Detection</h2>
        <div>
          <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">
            VAD Threshold: {{ config.vad_threshold.toFixed(2) }}
          </label>
          <input v-model.number="config.vad_threshold" type="range" min="0" max="1" step="0.05" class="w-full" />
        </div>
      </section>

      <!-- Custom Prompt -->
      <section class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
        <h2 class="text-sm font-semibold text-gray-500 uppercase mb-3">Custom System Prompt</h2>
        <textarea
          v-model="config.custom_system_prompt"
          rows="4"
          class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm font-mono"
          placeholder="Optional: override system prompt. Variables: {user_name} {current_time} {emotion}"
        ></textarea>
        <p class="text-xs text-gray-400 mt-1">Variables: {'{user_name}'} {'{current_time}'} {'{emotion}'}</p>
      </section>

      <!-- Save -->
      <div class="flex justify-end pb-8">
        <button
          @click="save"
          :disabled="saving"
          class="px-6 py-2.5 rounded-xl bg-blue-500 text-white text-sm font-medium disabled:opacity-40 hover:bg-blue-600 transition-all"
        >
          {{ saving ? 'Saving...' : 'Save Settings' }}
        </button>
      </div>
    </div>
  </div>
</template>
