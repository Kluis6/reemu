import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// Config afinada para Tauri v2 — ver https://v2.tauri.app/start/frontend/vite/
const host = process.env.TAURI_DEV_HOST

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],

  // Tauri espera uma porta fixa; falha em vez de cair pra outra.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: 'ws', host, port: 1421 }
      : undefined,
    watch: {
      // src-tauri é observado pelo próprio Tauri.
      ignored: ['**/src-tauri/**'],
    },
  },

  // Variáveis TAURI_ ficam disponíveis no frontend.
  envPrefix: ['VITE_', 'TAURI_ENV_*'],

  build: {
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
})
