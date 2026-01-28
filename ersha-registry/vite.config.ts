import { defineConfig } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import elmPlugin from 'vite-plugin-elm'

export default defineConfig({
  plugins: [
    tailwindcss(),
    elmPlugin()
  ],
})

