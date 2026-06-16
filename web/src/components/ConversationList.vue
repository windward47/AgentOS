<script setup lang="ts">
import { ref } from 'vue'
import { useAppStore } from '../stores/app'
import { useCompanion } from '../composables/useCompanion'
import type { ConversationMeta } from '../types/ipc'

defineProps<{ collapsed: boolean }>()
const emit = defineEmits<{ conversationSwitched: [] }>()

const store = useAppStore()
const { createConversation, switchConversation, deleteConversation, renameConversation, listConversations } = useCompanion()

const editingId = ref<string | null>(null)
const editTitle = ref('')

// ── Actions ──────────────────────────────────────────────────────────

async function doCreate() {
  try {
    const result = await createConversation()
    store.setCurrentConversationId(result.id)
    store.setMessages([])
    await refreshList()
    emit('conversationSwitched')
  } catch (e) { console.error('create conversation:', e) }
}

async function doSwitch(id: string) {
  if (store.currentConversationId === id) return
  try {
    const result = await switchConversation(id)
    store.setCurrentConversationId(id)
    store.setMessages(result.messages.map(m => ({ role: m.role as 'user' | 'assistant', content: m.content })))
    emit('conversationSwitched')
  } catch (e) { console.error('switch conversation:', e) }
}

async function doDelete(id: string) {
  if (!confirm('Delete this conversation?')) return
  try {
    const result = await deleteConversation(id)
    if (result.ok) {
      const wasCurrent = store.currentConversationId === id
      store.removeConversation(id)
      if (wasCurrent) {
        // Sidecar already switched to another conversation internally
        store.setCurrentConversationId(result.currentId)
        const switched = await switchConversation(result.currentId)
        store.setMessages(switched.messages.map(m => ({ role: m.role as 'user' | 'assistant', content: m.content })))
        emit('conversationSwitched')
      }
    }
  } catch (e) { console.error('delete conversation:', e) }
}

function startRename(conv: ConversationMeta) {
  editingId.value = conv.id
  editTitle.value = conv.title
}

async function commitRename(id: string) {
  const title = editTitle.value.trim()
  editingId.value = null
  if (!title) return
  try {
    await renameConversation(id, title)
    store.patchConversation(id, { title })
  } catch (e) { console.error('rename conversation:', e) }
}

function cancelRename() {
  editingId.value = null
}

async function refreshList() {
  try {
    const result = await listConversations()
    store.setConversations(result.conversations)
  } catch (e) { console.error('list conversations:', e) }
}

// Expose refresh for external use
defineExpose({ refreshList })

// ── Formatting ───────────────────────────────────────────────────────

function formatDate(iso: string): string {
  const d = new Date(iso)
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  if (diff < 86400000) return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  if (diff < 604800000) return d.toLocaleDateString([], { weekday: 'short' })
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' })
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- New conversation button -->
    <div class="px-2 pt-2 pb-1">
      <button @click="doCreate"
        class="flex items-center gap-2 w-full px-2 py-1.5 rounded-lg text-sm text-gray-600 hover:bg-white/80 transition-colors border border-dashed border-gray-200 hover:border-gray-300">
        <span class="text-base shrink-0">+</span>
        <span v-if="!collapsed" class="truncate">New Chat</span>
      </button>
    </div>

    <!-- Conversation list -->
    <div class="flex-1 overflow-y-auto px-2 py-1 space-y-0.5">
      <div v-for="conv in store.conversations" :key="conv.id"
        :class="[
          'group flex items-center gap-2 px-2 py-1.5 rounded-lg cursor-pointer transition-colors text-sm',
          store.currentConversationId === conv.id
            ? 'bg-white shadow-sm ring-1 ring-gray-200 text-gray-900 font-medium'
            : 'text-gray-600 hover:bg-white/60'
        ]"
        @click="doSwitch(conv.id)"
      >
        <!-- Icon -->
        <span class="text-xs shrink-0 opacity-60">💬</span>

        <!-- Title / Edit -->
        <div v-if="!collapsed" class="flex-1 min-w-0">
          <div v-if="editingId === conv.id" class="flex items-center gap-1">
            <input
              v-model="editTitle"
              class="flex-1 bg-transparent border-b border-blue-400 px-0.5 text-xs outline-none"
              @keydown.enter="commitRename(conv.id)"
              @keydown.escape="cancelRename()"
              @blur="commitRename(conv.id)"
              @click.stop
              autofocus
            />
          </div>
          <div v-else class="flex items-center gap-1 min-w-0">
            <span class="truncate text-xs">{{ conv.title }}</span>
            <span class="text-[10px] text-gray-300 shrink-0">{{ formatDate(conv.updatedAt) }}</span>
          </div>
        </div>

        <!-- Hover actions -->
        <div v-if="!collapsed" class="hidden group-hover:flex items-center gap-0.5 shrink-0">
          <button @click.stop="startRename(conv)"
            class="w-5 h-5 flex items-center justify-center rounded text-[10px] text-gray-400 hover:text-gray-600 hover:bg-gray-100"
            title="Rename">✎</button>
          <button @click.stop="doDelete(conv.id)"
            class="w-5 h-5 flex items-center justify-center rounded text-[10px] text-gray-400 hover:text-red-500 hover:bg-red-50"
            title="Delete">✕</button>
        </div>
      </div>

      <!-- Empty state -->
      <div v-if="!collapsed && store.conversations.length === 0" class="text-center py-8 text-xs text-gray-400">
        No conversations yet
      </div>
    </div>
  </div>
</template>
