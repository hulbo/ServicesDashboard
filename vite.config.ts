import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Tauri 개발 서버 권장 설정: 고정 포트, src-tauri 감시 제외
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    target: 'es2022',
  },
})
