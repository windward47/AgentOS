<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useCompanion } from '../composables/useCompanion'
import { useRouter } from 'vue-router'
import type { CompanionConfig, ProviderConfig } from '../types/ipc'

const router = useRouter()
const { getConfig, updateConfig, listModels } = useCompanion()
const config = ref<CompanionConfig | null>(null)
const saving = ref(false)
const saved = ref(false)

onMounted(async () => {
  // Show form immediately with defaults (avoid "Loading..." flash)
  config.value = {
    sandbox_path: '~/.companion/sandbox',
    llm_provider: 'siliconflow', asr_provider: 'xiaomi', tts_provider: 'xiaomi',
    system_mode: false, tts_auto_play: false, vad_threshold: 0.3,
    user_name: 'User', custom_system_prompt: '', live2d_model: 'haru', default_api_key: '', api_token: '',
    voice_mode: 'ptt', tts_voice: '茉莉', tts_speed: 1.0,
    llm: { provider: '', url: null, key: null, model: null },
    asr: { provider: '', url: null, key: null, model: null },
    tts: { provider: '', url: null, key: null, model: null },
    custom_providers: [],
    global_voice: {
      record_hotkey: 'Alt+`', tts_hotkey: 'Alt+T',
      inject_mode_switch_hotkey: 'Alt+Shift+V', engine_switch_hotkey: 'Alt+Shift+E',
      inject_mode: 'keyboard', asr_engine: 'mimo', tts_engine: 'mimo-tts', tts_voice: '茉莉', tts_speed: 1.0,
    },
  }
  try { config.value = await getConfig() } catch {}
  // Pre-fill provider fields for any preset that is selected
  if (config.value) {
    onProviderChange('llm')
    onProviderChange('asr')
    onProviderChange('tts')
  }
})

async function save() {
  if (!config.value) return
  saving.value = true; saved.value = false
  // Auto-save custom provider configs
  for (const kind of ['llm','asr','tts'] as const) {
    if (config.value[kind+'_provider' as keyof CompanionConfig] === 'custom' && config.value[kind].provider) {
      const existing = config.value.custom_providers.find(p => p.provider === config.value![kind].provider)
      if (!existing) config.value.custom_providers.push({ ...config.value[kind] })
    }
  }
  try { await updateConfig(config.value); saved.value = true; setTimeout(() => saved.value = false, 2000) }
  catch {} finally { saving.value = false }
}

/** Get saved provider names for datalist suggestions. */
function savedProviderNames(kind: 'llm' | 'asr' | 'tts'): string[] {
  if (!config.value) return []
  const names = config.value.custom_providers
    .filter(p => p.provider && p.provider !== '')
    .map(p => p.provider)
  const preset = config.value[kind].provider
  return [...new Set(preset ? [preset, ...names] : names)]
}

// ── Model detection (unified per kind) ──
const detecting = ref({ llm: false, asr: false, tts: false })
const modelLists = ref<Record<string, string[]>>({ llm: [], asr: [], tts: [] })

function modelList(kind: string) { return modelLists.value[kind] ?? [] }

async function detectModels(kind: 'llm' | 'asr' | 'tts') {
  if (!config.value) return
  const prov = config.value[kind]
  if (!prov.url) return
  detecting.value[kind] = true
  modelLists.value[kind] = []
  try {
    const models = await listModels(prov.url, prov.key || '')
    modelLists.value[kind] = models
    if (models.length > 0 && !prov.model) prov.model = models[0]
  } catch (err: any) { modelLists.value[kind] = [String(err)] }
  finally { detecting.value[kind] = false }
  // Auto-select first detected model as default voice for TTS
  if (kind === 'tts' && modelLists.value.tts.length > 0) {
    config.value!.tts_voice = modelLists.value.tts[0]
  }
}

// ── Provider UI helpers ──
const PRESET_URLS: Record<string, Record<string, string>> = {
  llm: { siliconflow: 'https://api.siliconflow.cn/v1', xiaomi: 'https://token-plan-cn.xiaomimimo.com/v1' },
  asr: { xiaomi: 'https://token-plan-cn.xiaomimimo.com/v1', local_funasr: 'http://localhost:8000/v1' },
  tts: { xiaomi: 'https://token-plan-cn.xiaomimimo.com/v1', local_cosyvoice: 'http://localhost:50002/v1' },
}
const TTS_VOICES: Record<string, string[]> = {
  xiaomi: ['mimo_default', '茉莉', '冰糖', '苏打', '白桦', 'Mia', 'Chloe', 'Milo', 'Dean'],
  local_cosyvoice: ['zh-CN-XiaoxiaoNeural', 'zh-CN-YunxiNeural', 'zh-CN-YunjianNeural', 'zh-CN-XiaoyiNeural'],
}
const PRESET_PROVIDERS: Record<string, string[]> = {
  llm: ['ollama', 'openai', 'local'],
  asr: ['whisper', 'azure', 'local'],
  tts: ['edge', 'azure', 'local'],
}

function isPreset(kind: string) { return config.value?.[kind+'_provider' as keyof CompanionConfig] !== 'custom' }
function showProviderDetail(_kind: string) { return true } // always show
function providerLabel(kind: string) { return isPreset(kind) ? (config.value?.[kind+'_provider' as keyof CompanionConfig] as string || 'Preset') : 'Custom' }
function providerPlaceholder(kind: string) { return isPreset(kind) ? '' : PRESET_PROVIDERS[kind]?.join(', ') || 'provider' }
function urlPlaceholder(kind: string) { return 'https://...' }
function providerPresets(kind: string) { return PRESET_PROVIDERS[kind] ?? [] }

/** Auto-fill provider name + URL when switching presets; clear on Custom. */
function onProviderChange(kind: 'llm' | 'asr' | 'tts') {
  if (!config.value) return
  const key = config.value[kind+'_provider' as keyof CompanionConfig] as string
  const urls = PRESET_URLS[kind] ?? {}
  if (key !== 'custom' && urls[key]) {
    config.value[kind].provider = key
    config.value[kind].url = urls[key]
  } else {
    config.value[kind].provider = ''
    config.value[kind].url = null
  }
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

    <div v-if="config" class="max-w-2xl mx-auto w-full px-6 py-6 space-y-6">

      <!-- ── AI Providers ── -->
      <section class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
        <div class="px-5 py-3 border-b border-gray-100 bg-gray-50/50">
          <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">AI Providers</h2>
        </div>

        <!-- Provider selector row -->
        <div class="p-5 grid grid-cols-3 gap-5">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">LLM Provider</label>
            <select v-model="config.llm_provider" @change="onProviderChange('llm')" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="siliconflow">SiliconFlow</option>
              <option value="xiaomi">Xiaomi</option>
              <option value="custom">Custom…</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">ASR Provider</label>
            <select v-model="config.asr_provider" @change="onProviderChange('asr')" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="xiaomi">Xiaomi</option>
              <option value="local_funasr">Local FunASR</option>
              <option value="custom">Custom…</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1.5">TTS Provider</label>
            <select v-model="config.tts_provider" @change="onProviderChange('tts')" class="block w-full rounded-lg border border-gray-200 px-3 py-2.5 text-sm shadow-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none transition-colors">
              <option value="xiaomi">Xiaomi</option>
              <option value="local_cosyvoice">Microsoft Edge (free)</option>
              <option value="custom">Custom…</option>
            </select>
          </div>
        </div>

        <!-- Provider detail fields (always visible, reused for presets too) -->
        <div class="px-5 pb-5 border-t border-gray-100 space-y-4 pt-4">
          <div>
            <label class="block text-[11px] font-medium text-gray-600 mb-1">Default API Key <span class="text-gray-400">(fallback for all providers, overridden by per-provider keys above)</span></label>
            <input v-model="config.default_api_key" type="password" placeholder="Or set COMPANION_API_TOKEN env var" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" />
          </div>
          <template v-for="kind in (['llm','asr','tts'] as const)" :key="kind">
            <div v-if="showProviderDetail(kind)" class="bg-gray-50 rounded-lg p-4 space-y-3">
              <h3 class="text-xs font-semibold text-gray-500 uppercase tracking-wider">{{ kind.toUpperCase() }} — {{ providerLabel(kind) }}</h3>
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-[11px] font-medium text-gray-600 mb-1">Provider Name</label>
                  <input v-model="config[kind].provider" :list="kind+'ProviderList'" :disabled="isPreset(kind)"
                    :class="['block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none', isPreset(kind) ? 'bg-gray-100 text-gray-500' : '']"
                    :placeholder="providerPlaceholder(kind)" />
                  <datalist :id="kind+'ProviderList'">
                    <option v-for="n in savedProviderNames(kind)" :key="n" :value="n" />
                    <option v-for="n in providerPresets(kind)" :key="n" :value="n" />
                  </datalist>
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-gray-600 mb-1">API URL</label>
                  <input v-model="config[kind].url" :disabled="isPreset(kind)"
                    :class="['block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none', isPreset(kind) ? 'bg-gray-100 text-gray-500' : '']"
                    :placeholder="urlPlaceholder(kind)" />
                </div>
              </div>
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-[11px] font-medium text-gray-600 mb-1">API Key</label>
                  <input v-model="config[kind].key" type="password" placeholder="sk-… or leave empty for env var"
                    class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-gray-600 mb-1">Model</label>
                  <div class="flex gap-2">
                    <button @click="detectModels(kind)" :disabled="detecting[kind]" class="shrink-0 text-xs px-3 py-2 rounded-lg bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-40">{{ detecting[kind] ? 'Detecting…' : 'Detect' }}</button>
                    <select v-if="modelList(kind).length" v-model="config[kind].model" class="flex-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono">
                      <option value="">— select —</option>
                      <option v-for="m in modelList(kind)" :key="m" :value="m">{{ m }}</option>
                    </select>
                    <input v-else v-model="config[kind].model" placeholder="model name" class="flex-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-mono focus:border-blue-400 focus:ring-2 focus:ring-blue-100 outline-none" />
                  </div>
                </div>
              </div>
            </div>
          </template>
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
              <option v-for="v in (TTS_VOICES[config.tts_provider] || TTS_VOICES['xiaomi'])" :key="v" :value="v">{{ v }}</option>
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
          <div><label class="block text-[11px] font-medium text-gray-600 mb-1">TTS Voice</label>
            <select v-model="config.global_voice.tts_voice" class="block w-full rounded-lg border border-gray-200 px-3 py-2 text-sm">
              <option v-for="v in (TTS_VOICES['xiaomi'] || [])" :key="v" :value="v">{{ v }}</option>
            </select>
          </div>
          <div><label class="block text-[11px] font-medium text-gray-600 mb-1">TTS Speed — {{ config.global_voice.tts_speed?.toFixed(1) || '1.0' }}</label>
            <input v-model.number="config.global_voice.tts_speed" type="range" min="0.5" max="2.0" step="0.1" class="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-500" />
          </div>
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
