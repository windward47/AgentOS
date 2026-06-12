<script setup lang="ts">
import { computed } from 'vue'
import { marked } from 'marked'

const props = defineProps<{ content: string }>()

const html = computed(() => {
  try {
    return marked.parse(props.content, { async: false }) as string
  } catch {
    return props.content
  }
})
</script>

<template>
  <div class="markdown-body text-[15px] leading-relaxed" v-html="html" />
</template>

<style scoped>
.markdown-body :deep(p) { margin: 0.4em 0; }
.markdown-body :deep(p:first-child) { margin-top: 0; }
.markdown-body :deep(p:last-child) { margin-bottom: 0; }
.markdown-body :deep(code) {
  background: rgba(0,0,0,0.06);
  padding: 0.15em 0.4em;
  border-radius: 4px;
  font-size: 0.9em;
  font-family: 'Cascadia Code', 'Fira Code', monospace;
}
.markdown-body :deep(pre) {
  background: rgba(0,0,0,0.06);
  padding: 0.8em 1em;
  border-radius: 8px;
  overflow-x: auto;
  margin: 0.6em 0;
}
.markdown-body :deep(pre code) {
  background: none;
  padding: 0;
  border-radius: 0;
}
.markdown-body :deep(ul), .markdown-body :deep(ol) {
  padding-left: 1.5em;
  margin: 0.4em 0;
}
.markdown-body :deep(li) { margin: 0.15em 0; }
.markdown-body :deep(h1), .markdown-body :deep(h2), .markdown-body :deep(h3),
.markdown-body :deep(h4), .markdown-body :deep(h5), .markdown-body :deep(h6) {
  margin: 0.6em 0 0.3em;
  font-weight: 600;
}
.markdown-body :deep(blockquote) {
  border-left: 3px solid #d1d5db;
  padding-left: 0.8em;
  color: #6b7280;
  margin: 0.4em 0;
}
.markdown-body :deep(a) { color: #3b82f6; text-decoration: underline; }
.markdown-body :deep(table) {
  border-collapse: collapse;
  margin: 0.6em 0;
  font-size: 0.9em;
}
.markdown-body :deep(th), .markdown-body :deep(td) {
  border: 1px solid #d1d5db;
  padding: 0.3em 0.6em;
  text-align: left;
}
.markdown-body :deep(th) { background: #f3f4f6; font-weight: 600; }
.markdown-body :deep(hr) { border: none; border-top: 1px solid #e5e7eb; margin: 0.8em 0; }
</style>
