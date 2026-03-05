import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    host: '0.0.0.0',
    port: 6000,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:6001',
        changeOrigin: true
      },
      '/term': {
        target: 'http://127.0.0.1:6001',
        changeOrigin: true,
        ws: true
      }
    }
  }
})
