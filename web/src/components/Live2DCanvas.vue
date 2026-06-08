<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { Application, Container, Graphics, Text, TextStyle } from 'pixi.js'
import { Live2DModel } from 'pixi-live2d-display'

const canvasRef = ref<HTMLDivElement>()
const modelLoaded = ref(false)
const modelError = ref('')

let app: Application | null = null
let live2dModel: Live2DModel | null = null
let unlistenAudio: (() => void) | null = null
let unlistenState: (() => void) | null = null

// ---------------------------------------------------------------------------
// Fallback avatar when no model is loaded
// ---------------------------------------------------------------------------
function drawFallbackAvatar(container: Container) {
  const g = new Graphics()
  // Circle head
  g.beginFill(0x6b8ef0)
  g.drawCircle(160, 100, 70)
  g.endFill()

  // Eyes
  g.beginFill(0xffffff)
  g.drawCircle(140, 85, 18)
  g.drawCircle(180, 85, 18)
  g.endFill()
  g.beginFill(0x333333)
  g.drawCircle(143, 85, 8)
  g.drawCircle(183, 85, 8)
  g.endFill()

  // Mouth (simple arc)
  g.lineStyle(3, 0x333333)
  g.arc(160, 120, 20, 0.2, Math.PI - 0.2)

  container.addChild(g)

  const style = new TextStyle({ fill: '#666', fontSize: 14, fontFamily: 'sans-serif' })
  const label = new Text('加载 Live2D 模型以显示互动形象', style)
  label.anchor.set(0.5, 0)
  label.x = 160
  label.y = 200
  container.addChild(label)
}

// ---------------------------------------------------------------------------
// Model loading
// ---------------------------------------------------------------------------
async function loadModel(path: string) {
  if (!app) return
  try {
    live2dModel = await Live2DModel.from(path)
    live2dModel.anchor.set(0.5, 0.5)
    live2dModel.position.set(app.screen.width / 2, app.screen.height / 2)
    live2dModel.scale.set(0.3)
    app.stage.addChild(live2dModel as any)
    modelLoaded.value = true
    modelError.value = ''
  } catch (e: any) {
    modelError.value = `模型加载失败: ${e?.message || e}`
    drawFallbackAvatar(app.stage)
  }
}

// ---------------------------------------------------------------------------
// Event listeners
// ---------------------------------------------------------------------------
async function setupListeners() {
  // Audio level → mouth open
  unlistenAudio = await listen<{ level: number }>('audio_level', (event) => {
    if (live2dModel) {
      (live2dModel.internalModel.coreModel as any).setParameterValueById(
        'ParamMouthOpenY',
        Math.min(event.payload.level * 1.5, 1.0),
      )
    }
  })

  // Agent state → animation
  unlistenState = await listen<{ state: string }>('agent_state', (event) => {
    const state = event.payload.state
    if (live2dModel) {
      try {
        live2dModel.motion(state)
      } catch {
        // motion group not found, ignore
      }
    }
  })
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------
onMounted(async () => {
  if (!canvasRef.value) return

  app = new Application({
    width: 320,
    height: 320,
    backgroundColor: 0xf0f4ff,
    antialias: true,
    resolution: window.devicePixelRatio || 1,
    autoDensity: true,
  })

  canvasRef.value.appendChild(app.view as HTMLCanvasElement)

  // Try loading model from the default path
  const modelPath = `${window.location.origin}/models/Hiyori/Hiyori.model3.json`
  await loadModel(modelPath)

  await setupListeners()
})

onBeforeUnmount(() => {
  unlistenAudio?.()
  unlistenState?.()
  live2dModel?.destroy()
  app?.destroy(true)
})
</script>

<template>
  <div
    ref="canvasRef"
    class="live2d-container relative flex items-center justify-center"
    :class="{ 'min-h-[320px]': true }"
  >
    <!-- Loading indicator -->
    <div
      v-if="!modelLoaded && !modelError"
      class="absolute inset-0 flex items-center justify-center text-gray-400 text-sm"
    >
      <div class="text-center">
        <div class="animate-spin w-6 h-6 border-2 border-blue-400 border-t-transparent rounded-full mx-auto mb-2"></div>
        <span>加载模型中...</span>
      </div>
    </div>

    <!-- Error / fallback hint -->
    <div
      v-if="modelError"
      class="absolute bottom-2 left-2 right-2 text-xs text-gray-400 text-center px-2"
    >
      {{ modelError }}
    </div>
  </div>
</template>

<style scoped>
.live2d-container {
  canvas {
    display: block;
  }
}
</style>
