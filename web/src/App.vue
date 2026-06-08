<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const systemMode = ref(false)

onMounted(async () => {
  try {
    const cfg: { system_mode: boolean } = await invoke('get_config')
    systemMode.value = cfg.system_mode
  } catch { /* ignore */ }
})
</script>

<template>
  <div class="flex flex-col h-screen bg-gray-50 dark:bg-gray-900">
    <div class="flex-1 min-h-0">
      <RouterView />
    </div>
    <!-- Status bar -->
    <div class="flex items-center justify-between px-4 py-1.5 text-xs border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-500">
      <div class="flex items-center gap-3">
        <span :class="systemMode ? 'text-orange-500' : 'text-green-500'">
          {{ systemMode ? 'Unrestricted' : 'Sandbox' }}
        </span>
      </div>
      <div class="flex items-center gap-3">
        <a href="/settings" class="hover:text-gray-700 dark:hover:text-gray-300 transition-colors">Settings</a>
      </div>
    </div>
  </div>
</template>
