<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'

interface Config {
  sandbox_path: string; llm_provider: string; asr_provider: string; tts_provider: string
  system_mode: boolean; enable_accessibility: boolean; vad_threshold: number
  user_name: string; style_template: string; custom_system_prompt: string | null
  emotion_mapping: Record<string, string>; api_token: string | null
  voice_mode: string; tts_voice: string
}

const router = useRouter()
const config = ref<Config | null>(null)
const saving = ref(false)
const saved = ref(false)

onMounted(async () => {
  try { config.value = await invoke<Config>('get_config') } catch {
    // Running without Tauri — use defaults
    config.value = {
      sandbox_path: '~/.companion/sandbox', llm_provider: 'siliconflow', asr_provider: 'local', tts_provider: 'local',
      system_mode: false, enable_accessibility: false, vad_threshold: 0.3,
      user_name: 'User', style_template: 'professional', custom_system_prompt: null,
      emotion_mapping: {}, api_token: null,
      voice_mode: 'ptt', tts_voice: '茉莉',
    }
  }
})

async function save() {
  if (!config.value) return
  saving.value = true; saved.value = false
  try { await invoke('update_config', { config: config.value }); saved.value = true; setTimeout(() => saved.value = false, 2000) }
  catch {} finally { saving.value = false }
}
</script>

<template>
  <div class="flex flex-col h-full overflow-y-auto bg-gray-50">
    <!-- Header -->
    <div class="sticky top-0 z-10 flex items-center justify-between px-6 py-4 bg-white border-b border-gray-200">
      <h1 class="text-lg font-semibold text-gray-900">Settings</h1>
      <div class="flex items-center gap-3">
        <span v-if="saved" class="text-xs text-emerald-600 font-medium bg-emerald-50 px-2 py-1 rounded-full">✓ Saved</span>
        <button @click="router.push('/')" class="text-sm text-gray-500 hover:text-gray-700 px-3 py-1.5 rounded-lg border border-gray-200 hover:bg-gray-50 transition-colors">← Back</button>
      </div>
    </div>

    <div v-if="!config" class="flex-1 flex items-center justify-center text-gray-400">Loading...</div>

    <!-- Sections — Tailwind Plus Form Layout style: stacked card sections -->
    <div v-else class="max-w-2xl mx-auto w-full px-6 py-6 space-y-6">
      <!-- Profile -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Profile</h2>
        </div>
        <div class="p-5 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">User Name</label>
            <input v-model="config.user_name" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm shadow-sm placeholder:text-gray-400 focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">Conversation Style</label>
            <select v-model="config.style_template" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="professional">Professional</option>
              <option value="humorous">Humorous</option>
              <option value="gentle">Gentle Companion</option>
              <option value="geek">Hardcore Geek</option>
            </select>
          </div>
        </div>
      </section>

      <!-- Providers -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">AI Providers</h2>
        </div>
        <div class="p-5 grid grid-cols-3 gap-5">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">LLM Model</label>
            <select v-model="config.llm_provider" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="siliconflow">Nex-N2-Pro 🆓</option>
              <option value="xiaomi">MiMo V2.5 Pro</option>
              <option value="openai">OpenAI</option>
              <option value="ollama">Ollama (Local)</option>
              <option value="claude">Claude</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">ASR (Speech)</label>
            <select v-model="config.asr_provider" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="xiaomi">Xiaomi (MiMo)</option>
              <option value="local">Whisper (Local)</option>
              <option value="cloud">Whisper (Cloud)</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">TTS (Voice)</label>
            <select v-model="config.tts_provider" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="xiaomi">Xiaomi (MiMo)</option>
              <option value="local">ChatTTS (Local)</option>
              <option value="cloud">Azure TTS</option>
            </select>
          </div>
        </div>
      </section>

      <!-- Voice & Speech -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Voice & Speech</h2>
        </div>
        <div class="p-5 space-y-4">
          <div class="flex items-center justify-between py-1">
            <div>
              <div class="text-sm font-medium text-gray-900">TTS Always-On</div>
              <div class="text-xs text-gray-500 mt-0.5">Auto-play voice for every AI reply</div>
            </div>
            <button type="button" @click="config.enable_accessibility = !config.enable_accessibility"
              :class="['relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 focus:outline-none',
                       config.enable_accessibility ? 'bg-emerald-500' : 'bg-gray-200']">
              <span :class="['pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200',
                            config.enable_accessibility ? 'translate-x-5' : 'translate-x-0']" />
            </button>
          </div>
          <div class="flex items-center justify-between py-1">
            <div>
              <div class="text-sm font-medium text-gray-900">Voice Mode</div>
              <div class="text-xs text-gray-500 mt-0.5">Auto-stop recording on silence vs. manual push-to-talk</div>
            </div>
            <div class="flex rounded-lg border border-gray-200 bg-gray-50 text-[11px] overflow-hidden">
              <button type="button" @click="config.voice_mode = 'auto'"
                :class="['px-3 py-1.5 font-medium transition-colors', config.voice_mode === 'auto' ? 'bg-blue-500 text-white' : 'text-gray-500 hover:text-gray-700']">Auto</button>
              <button type="button" @click="config.voice_mode = 'ptt'"
                :class="['px-3 py-1.5 font-medium transition-colors', config.voice_mode === 'ptt' ? 'bg-blue-500 text-white' : 'text-gray-500 hover:text-gray-700']">PTT</button>
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">TTS Voice</label>
            <select v-model="config.tts_voice" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="mimo_default">mimo_default</option>
              <option value="茉莉">茉莉</option>
              <option value="冰糖">冰糖</option>
              <option value="苏打">苏打</option>
              <option value="白桦">白桦</option>
              <option value="Mia">Mia</option>
              <option value="Chloe">Chloe</option>
              <option value="Milo">Milo</option>
              <option value="Dean">Dean</option>
            </select>
          </div>
          <label class="block text-sm font-medium text-gray-700 mb-2">
            VAD Threshold — <span class="text-gray-500 font-normal">{{ config.vad_threshold.toFixed(2) }}</span>
          </label>
          <input v-model.number="config.vad_threshold" type="range" min="0" max="1" step="0.05"
            class="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-500" />
          <div class="flex justify-between text-[11px] text-gray-400 mt-1"><span>Sensitive</span><span>Strict</span></div>
        </div>
      </section>

      <!-- Sandbox -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Sandbox & Security</h2>
        </div>
        <div class="p-5 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">Sandbox Path</label>
            <input v-model="config.sandbox_path" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm font-mono shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">API Token (Xiaomi / Cloud)</label>
            <input v-model="config.api_token" type="password" placeholder="Or set COMPANION_API_TOKEN env var"
              class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm font-mono shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors" />
            <p class="text-xs text-gray-400 mt-1">Stored in ~/.companion/config.json. Never committed to git.</p>
          </div>
          <div class="flex items-center justify-between py-1">
            <div>
              <div class="text-sm font-medium text-gray-900">System Mode</div>
              <div class="text-xs text-gray-500 mt-0.5">Allow unrestricted file system access</div>
            </div>
            <button type="button" @click="config!.system_mode = !config!.system_mode"
              :class="['relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 focus:outline-none',
                       config.system_mode ? 'bg-orange-500' : 'bg-gray-200']">
              <span :class="['pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200',
                            config.system_mode ? 'translate-x-5' : 'translate-x-0']" />
            </button>
          </div>
        </div>
      </section>


      <!-- Custom System Prompt -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Custom System Prompt</h2>
        </div>
        <div class="p-5">
          <textarea v-model="config.custom_system_prompt" rows="4" placeholder="Optional — override the system prompt.&#10;Variables: {user_name} {current_time} {emotion}"
            class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm font-mono shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors resize-y" />
          <p class="text-xs text-gray-400 mt-2">Variables: {'{user_name}'} {'{current_time}'} {'{emotion}'}</p>
        </div>
      </section>

      <!-- Save button -->
      <div class="flex justify-end pb-8">
        <button @click="save" :disabled="saving"
          class="inline-flex items-center px-5 py-2.5 rounded-lg bg-blue-500 text-sm font-medium text-white shadow-sm hover:bg-blue-600 disabled:opacity-40 disabled:hover:bg-blue-500 transition-colors">
          {{ saving ? 'Saving...' : 'Save Settings' }}
        </button>
      </div>
    </div>
  </div>
</template>
