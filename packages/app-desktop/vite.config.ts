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
    // Sem TAURI_DEV_HOST, escuta explicitamente em 127.0.0.1 — o `devUrl` do
    // Tauri também é 127.0.0.1. O WebKitGTK resolve `localhost` como `::1`
    // primeiro e, como o Vite não escuta IPv6, dava "Connection refused" na
    // webview (o Chrome cai pra IPv4 sozinho, por isso funcionava lá).
    host: host || '127.0.0.1',
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
