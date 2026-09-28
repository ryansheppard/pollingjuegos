import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    host: 'localhost',
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8000',
        headers: { host: 'localhost:5173' },
      },
      '/accounts': {
        target: 'http://127.0.0.1:8000',
        headers: { host: 'localhost:5173' },
      },
    },
  },
})
