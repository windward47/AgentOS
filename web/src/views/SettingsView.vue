<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import type { CompanionConfig, ProviderConfig } from '../types/ipc'

const router = useRouter()
const config = ref<CompanionConfig | null>(null)
const saving = ref(false)
const saved = ref(false)

onMounted(async () => {
  try { config.value = await invoke<CompanionConfig>('get_config') } catch {
    config.value = {
      sandbox_path: '~/.companion/sandbox',
      llm_provider: 'siliconflow', asr_provider: 'xiaomi', tts_provider: 'xiaomi',
      system_mode: false, tts_auto_play: false, vad_threshold: 0.3,
      user_name: 'User', custom_system_prompt: null, api_token: null,
      voice_mode: 'ptt', tts_voice: '茉莉', tts_speed: 1.0,
      llm: { url: null, key: null, model: null },
      asr: { url: null, key: null, model: null },
      tts: { url: null, key: null, model: null },
      global_voice: {
        record_hotkey: 'Alt+`', tts_hotkey: 'Alt+T',
        inject_mode_switch_hotkey: 'Alt+Shift+V', engine_switch_hotkey: 'Alt+Shift+E',
        inject_mode: 'keyboard', asr_engine: 'mimo', tts_engine: 'mimo-tts',
      },
    }
  }
})

async function save() {
  if (!config.value) return
  saving.value = true; saved.value = false
  try { await invoke('update_config', { newConfig: config.value }); saved.value = true; setTimeout(() => saved.value = false, 2000) }
  catch {} finally { saving.value = false }
}

// ── Model detection ──
const detectingLlm = ref(false); const detectingAsr = ref(false); const detectingTts = ref(false)
const llmModels = ref<string[]>([]); const asrModels = ref<string[]>([]); const ttsModels = ref<string[]>([])

async function detectModels(kind: 'llm' | 'asr' | 'tts') {
  if (!config.value) return
  const prov = config.value[kind]
  if (!prov.url) return
  const setDetecting = kind === 'llm' ? (v: boolean) => detectingLlm.value = v
    : kind === 'asr' ? (v: boolean) => detectingAsr.value = v : (v: boolean) => detectingTts.value = v
  const setModels = kind === 'llm' ? llmModels : kind === 'asr' ? asrModels : ttsModels
  setDetecting(true); setModels.value = []
  try {
    const models = await invoke<string[]>('list_models', { baseUrl: prov.url, apiKey: prov.key || '' })
    setModels.value = models
    if (models.length > 0 && !prov.model) prov.model = models[0]
  } catch (err: any) { setModels.value = [String(err)] }
  finally { setDetecting(false) }
}
</script>

<template>
  <div class="flex flex-col h-full overflow-y-auto bg-gray-50">
    <div class="sticky top-0 z-10 flex items-center justify-between px-6 py-4 bg-white border-b border-gray-200">
      <h1 class="text-lg font-semibold text-gray-900">Settings</h1>
      <div class="flex items-center gap-3">
        <span v-if="saved" class="text-xs text-emerald-600 font-medium bg-emerald-50 px-2 py-1 rounded-full">✓ Saved</span>
        <button @click="router.push('/')" class="text-sm text-gray-500 hover:text-gray-700 px-3 py-1.5 rounded-lg border border-gray-200 hover:bg-gray-50 transition-colors">← Back</button>
      </div>
    </div>

    <div v-if="!config" class="flex-1 flex items-center justify-center text-gray-400">Loading...</div>

    <div v-else class="max-w-2xl mx-auto w-full px-6 py-6 space-y-6">

      <!-- ── AI Providers ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">AI Providers</h2>
        </div>
        <div class="p-5 grid grid-cols-3 gap-5">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">LLM</label>
            <select v-model="config.llm_provider" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="siliconflow">MiMo V2.5</option>
              <option value="xiaomi">MiMo V2.5 Pro</option>
              <option value="custom">Custom API</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">ASR</label>
            <select v-model="config.asr_provider" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="xiaomi">Xiaomi MiMo</option>
              <option value="custom">Custom API</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">TTS</label>
            <select v-model="config.tts_provider" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="xiaomi">Xiaomi MiMo</option>
              <option value="custom">Custom API</option>
            </select>
          </div>
        </div>

        <!-- Custom provider fields -->
        <div v-if="config.llm_provider === 'custom' || config.asr_provider === 'custom' || config.tts_provider === 'custom'" class="px-5 pb-5 border-t border-gray-100 space-y-5 pt-4">
          <div v-if="config.llm_provider === 'custom'">
            <h3 class="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-3">Custom LLM</h3>
            <div class="grid grid-cols-2 gap-3 mb-3">
              <div><label class="block text-[11px] font-medium text-gray-600 mb-1">API URL</label><input v-model="config.llm.url" placeholder="https://api.openai.com/v1" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /></div>
              <div><label class="block text-[11px] font-medium text-gray-600 mb-1">API Key</label><input v-model="config.llm.key" type="password" placeholder="sk-..." class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /></div>
            </div>
            <div class="flex gap-2"><input v-model="config.llm.model" placeholder="Model name" class="flex-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /><button @click="detectModels('llm')" :disabled="detectingLlm" class="shrink-0 text-xs px-3 py-2 rounded-lg bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-40">{{ detectingLlm ? '...' : 'Detect' }}</button></div>
            <select v-if="llmModels.length" v-model="config.llm.model" class="block w-full mt-2 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono"><option v-for="m in llmModels" :key="m" :value="m">{{ m }}</option></select>
          </div>
          <div v-if="config.asr_provider === 'custom'">
            <h3 class="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-3">Custom ASR</h3>
            <div class="grid grid-cols-2 gap-3 mb-3">
              <div><label class="block text-[11px] font-medium text-gray-600 mb-1">API URL</label><input v-model="config.asr.url" placeholder="https://api.example.com/v1" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /></div>
              <div><label class="block text-[11px] font-medium text-gray-600 mb-1">API Key</label><input v-model="config.asr.key" type="password" placeholder="sk-..." class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /></div>
            </div>
            <div class="flex gap-2"><input v-model="config.asr.model" placeholder="Model name" class="flex-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /><button @click="detectModels('asr')" :disabled="detectingAsr" class="shrink-0 text-xs px-3 py-2 rounded-lg bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-40">{{ detectingAsr ? '...' : 'Detect' }}</button></div>
            <select v-if="asrModels.length" v-model="config.asr.model" class="block w-full mt-2 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono"><option v-for="m in asrModels" :key="m" :value="m">{{ m }}</option></select>
          </div>
          <div v-if="config.tts_provider === 'custom'">
            <h3 class="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-3">Custom TTS</h3>
            <div class="grid grid-cols-2 gap-3 mb-3">
              <div><label class="block text-[11px] font-medium text-gray-600 mb-1">API URL</label><input v-model="config.tts.url" placeholder="https://api.example.com/v1" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /></div>
              <div><label class="block text-[11px] font-medium text-gray-600 mb-1">API Key</label><input v-model="config.tts.key" type="password" placeholder="sk-..." class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /></div>
            </div>
            <div class="flex gap-2"><input v-model="config.tts.model" placeholder="Model name" class="flex-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" /><button @click="detectModels('tts')" :disabled="detectingTts" class="shrink-0 text-xs px-3 py-2 rounded-lg bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-40">{{ detectingTts ? '...' : 'Detect' }}</button></div>
            <select v-if="ttsModels.length" v-model="config.tts.model" class="block w-full mt-2 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono"><option v-for="m in ttsModels" :key="m" :value="m">{{ m }}</option></select>
          </div>
        </div>
      </section>

      <!-- ── Voice & TTS ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Voice &amp; TTS</h2>
        </div>
        <div class="p-5 space-y-4">
          <div class="flex items-center justify-between py-1">
            <div><div class="text-sm font-medium text-gray-900">TTS Auto-Play</div><div class="text-xs text-gray-500 mt-0.5">Auto-read every AI reply aloud</div></div>
            <button @click="config.tts_auto_play = !config.tts_auto_play" :class="['relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors', config.tts_auto_play ? 'bg-emerald-500' : 'bg-gray-200']"><span :class="['pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200', config.tts_auto_play ? 'translate-x-5' : 'translate-x-0']" /></button>
          </div>
          <div class="flex items-center justify-between py-1">
            <div><div class="text-sm font-medium text-gray-900">Voice Mode</div><div class="text-xs text-gray-500 mt-0.5">Auto-stop on silence vs. push-to-talk</div></div>
            <div class="flex rounded-lg border border-gray-200 bg-gray-50 text-[11px] overflow-hidden">
              <button @click="config.voice_mode = 'auto'" :class="['px-3 py-1.5 font-medium transition-colors', config.voice_mode === 'auto' ? 'bg-blue-500 text-white' : 'text-gray-500 hover:text-gray-700']">Auto</button>
              <button @click="config.voice_mode = 'ptt'" :class="['px-3 py-1.5 font-medium transition-colors', config.voice_mode === 'ptt' ? 'bg-blue-500 text-white' : 'text-gray-500 hover:text-gray-700']">PTT</button>
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">TTS Voice</label>
            <select v-model="config.tts_voice" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none">
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
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">TTS Speed — <span class="text-gray-500 font-normal">{{ config.tts_speed.toFixed(1) }}</span></label>
            <input v-model.number="config.tts_speed" type="range" min="0.5" max="2.0" step="0.1" class="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">VAD Threshold — <span class="text-gray-500 font-normal">{{ config.vad_threshold.toFixed(2) }}</span></label>
            <input v-model.number="config.vad_threshold" type="range" min="0" max="1" step="0.05" class="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-500" />
            <div class="flex justify-between text-[11px] text-gray-400 mt-1"><span>Sensitive</span><span>Strict</span></div>
          </div>
        </div>
      </section>

      <!-- ── Hotkeys ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Global Hotkeys</h2>
        </div>
        <div class="p-5 grid grid-cols-2 gap-4">
          <div><label class="block text-[11px] font-medium text-gray-600 mb-1">Record (ASR)</label><input v-model="config.global_voice.record_hotkey" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-sm font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" placeholder="Alt+`" /></div>
          <div><label class="block text-[11px] font-medium text-gray-600 mb-1">TTS Selected</label><input v-model="config.global_voice.tts_hotkey" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-sm font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" placeholder="Alt+T" /></div>
          <div><label class="block text-[11px] font-medium text-gray-600 mb-1">Switch Inject</label><input v-model="config.global_voice.inject_mode_switch_hotkey" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-sm font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" placeholder="Alt+Shift+V" /></div>
          <div><label class="block text-[11px] font-medium text-gray-600 mb-1">Switch ASR Engine</label><input v-model="config.global_voice.engine_switch_hotkey" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-sm font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" placeholder="Alt+Shift+E" /></div>
        </div>
      </section>

      <!-- ── Sandbox & Security ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Sandbox &amp; Security</h2>
        </div>
        <div class="p-5 space-y-4">
          <div><label class="block text-sm font-medium text-gray-700 mb-1.5">Sandbox Path</label><input v-model="config.sandbox_path" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm font-mono shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors" /></div>
          <div><label class="block text-sm font-medium text-gray-700 mb-1.5">API Token</label><input v-model="config.api_token" type="password" placeholder="Or set COMPANION_API_TOKEN env var" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm font-mono shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors" /></div>
          <div class="flex items-center justify-between py-1">
            <div><div class="text-sm font-medium text-gray-900">System Mode</div><div class="text-xs text-gray-500 mt-0.5">Unrestricted file system access</div></div>
            <button @click="config!.system_mode = !config!.system_mode" :class="['relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors', config.system_mode ? 'bg-orange-500' : 'bg-gray-200']"><span :class="['pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200', config.system_mode ? 'translate-x-5' : 'translate-x-0']" /></button>
          </div>
        </div>
      </section>

      <!-- ── Profile ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Profile</h2>
        </div>
        <div class="p-5"><label class="block text-sm font-medium text-gray-700 mb-1.5">User Name</label><input v-model="config.user_name" class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors" placeholder="User" /></div>
      </section>

      <!-- ── System Prompt ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">System Prompt</h2>
        </div>
        <div class="p-5">
          <textarea v-model="config.custom_system_prompt" rows="4" placeholder="Optional — override the system prompt sent to the AI.&#10;Leave empty to use default." class="block w-full rounded-lg border border-gray-200 px-3.5 py-2.5 text-sm font-mono shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors resize-y" />
        </div>
      </section>

      <div class="flex justify-end pb-8">
        <button @click="save" :disabled="saving" class="inline-flex items-center px-5 py-2.5 rounded-lg bg-blue-500 text-sm font-medium text-white shadow-sm hover:bg-blue-600 disabled:opacity-40 transition-colors">{{ saving ? 'Saving...' : 'Save Settings' }}</button>
      </div>
    </div>
  </div>
</template>
