import { fileURLToPath, URL } from 'node:url'
import vue from '@vitejs/plugin-vue'
import frappeui from 'frappe-ui/vite'
import { defineConfig } from 'vitest/config'

const isPreview = process.env.PILOT_DESKTOP_PREVIEW === '1'
const previewAliases: Record<string, string> = isPreview
  ? {
      '@tauri-apps/api/core': fileURLToPath(new URL('./preview/tauri-core.ts', import.meta.url)),
      '@tauri-apps/api/event': fileURLToPath(new URL('./preview/tauri-event.ts', import.meta.url)),
    }
  : {}

export default defineConfig({
  plugins: [frappeui({ frappeProxy: false, jinjaBootData: false, buildConfig: false }), vue()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)), ...previewAliases },
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  build: { outDir: 'dist', target: 'safari16', sourcemap: false },
  test: { environment: 'node', include: ['src/**/*.test.ts', 'scripts/**/*.test.mjs'] },
})
