<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useCompanion } from './composables/useCompanion'
import { useRouter, useRoute } from 'vue-router'

const router = useRouter()
const route = useRoute()
const mode = ref(false)
const collapsed = ref(false)
const { getConfig } = useCompanion()

const active = computed(() => {
  if (route.path === '/settings') return 'settings'
  if (route.path === '/live2d') return 'live2d'
  return 'chat'
})
// Avatar window: no sidebar, full-screen transparent background
const isAvatarWindow = computed(() => route.path === '/avatar')

onMounted(async () => {
  try { const c = await getConfig(); mode.value = c.system_mode } catch {}
})

function go(to: string) { router.push(to) }
</script>

<template>
  <!-- Avatar window: no sidebar, transparent -->
  <div v-if="isAvatarWindow" class="h-screen w-screen bg-transparent">
    <RouterView />
  </div>

  <!-- Main window: sidebar + content -->
  <div v-else class="flex h-screen w-screen bg-white text-gray-900">
    <aside :class="['flex flex-col border-r border-gray-200 bg-gray-50 shrink-0 transition-all', collapsed ? 'w-14' : 'w-[220px]']">
      <div class="flex items-center gap-2 px-4 h-14 border-b border-gray-200">
        <span class="text-lg">🤖</span>
        <span v-if="!collapsed" class="font-semibold text-sm">Companion</span>
      </div>
      <nav class="flex-1 py-2 space-y-0.5 px-2">
        <button @click="go('/')"
          :class="['flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm transition-colors',
                   active === 'chat' ? 'bg-white shadow-sm ring-1 ring-gray-200 text-gray-900 font-medium' : 'text-gray-600 hover:bg-white/60']">
          <span class="text-base">💬</span>
          <span v-if="!collapsed">Chat</span>
        </button>
        <button @click="go('/live2d')"
          :class="['flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm transition-colors',
                   active === 'live2d' ? 'bg-white shadow-sm ring-1 ring-gray-200 text-gray-900 font-medium' : 'text-gray-600 hover:bg-white/60']">
          <span class="text-base">🧑</span>
          <span v-if="!collapsed">Live2D</span>
        </button>
        <button @click="go('/settings')"
          :class="['flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm transition-colors',
                   active === 'settings' ? 'bg-white shadow-sm ring-1 ring-gray-200 text-gray-900 font-medium' : 'text-gray-600 hover:bg-white/60']">
          <span class="text-base">⚙️</span>
          <span v-if="!collapsed">Settings</span>
        </button>
      </nav>
      <div class="px-2 py-3 border-t border-gray-200 space-y-1">
        <div v-if="!collapsed" class="flex items-center gap-1.5 px-2 text-[11px]" :class="mode ? 'text-orange-500' : 'text-green-600'">
          <span class="w-1.5 h-1.5 rounded-full" :class="mode ? 'bg-orange-500' : 'bg-green-500'" />
          {{ mode ? 'Unrestricted' : 'Sandbox' }}
        </div>
        <button @click="collapsed = !collapsed" class="text-xs text-gray-400 hover:text-gray-500 px-2 py-1 w-full text-left">
          {{ collapsed ? '☰' : '←' }}
        </button>
      </div>
    </aside>
    <main class="flex-1 flex flex-col min-w-0 overflow-hidden bg-white">
      <RouterView />
    </main>
  </div>
</template>
