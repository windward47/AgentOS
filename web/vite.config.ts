import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'
import { homedir } from 'os'
import { existsSync, readFileSync } from 'fs'

const HOME_MODELS = resolve(homedir(), '.companion', 'models')

/** Vite plugin: serve live2d models from ~/.companion/models/ as fallback */
function live2dModelServer(): any {
  return {
    name: 'live2d-model-server',
    configureServer(server: any) {
      server.middlewares.use('/live2d/models', (req: any, res: any, next: any) => {
        const urlPath = decodeURIComponent(req.url || '/')
        const localPath = resolve(HOME_MODELS, urlPath.replace(/^\//, ''))
        // Only handle if file exists in ~/.companion/models/
        if (existsSync(localPath) && !localPath.includes('..')) {
          try {
            const data = readFileSync(localPath)
            const ext = localPath.split('.').pop() || ''
            const mime: Record<string, string> = { json: 'application/json', png: 'image/png', moc3: 'application/octet-stream', wav: 'audio/wav', can3: 'application/octet-stream', cmo3: 'application/octet-stream' }
            res.setHeader('Content-Type', mime[ext] || 'application/octet-stream')
            res.setHeader('Access-Control-Allow-Origin', '*')
            res.end(data)
            return
          } catch {}
        }
        next()
      })
    },
  }
}

export default defineConfig({
  plugins: [vue(), tailwindcss(), live2dModelServer()],
  clearScreen: false,
  server: {
    port: 5173, strictPort: true,
    watch: { ignored: ['**/src-tauri/**', '**/companion-core/**', '**/companion-tauri/**'] },
    fs: { allow: ['.', HOME_MODELS] },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        avatar: resolve(__dirname, 'avatar.html'),
      },
    },
  },
})
