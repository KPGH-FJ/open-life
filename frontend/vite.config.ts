import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const vitePort = Number(process.env.PORT || process.env.VITE_PORT || 5173)

export default defineConfig(async ({ command }) => ({
  base: command === 'build' ? './' : '/',
  plugins: [react()],
  resolve: {
    alias: {
      "@": new URL("./src", import.meta.url).pathname,
    },
  },
  clearScreen: false,
  server: {
    port: vitePort,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
  },
}))
