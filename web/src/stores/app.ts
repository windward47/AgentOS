import { defineStore } from 'pinia'
import { ref } from 'vue'

/** Global application state accessible from any Vue component. */
export const useAppStore = defineStore('app', () => {
  const isConnected = ref(false)
  const agentState = ref<'idle' | 'thinking' | 'speaking' | 'listening'>('idle')

  function setAgentState(state: typeof agentState.value) {
    agentState.value = state
  }

  return { isConnected, agentState, setAgentState }
})
