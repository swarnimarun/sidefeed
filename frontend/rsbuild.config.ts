import { defineConfig } from '@rsbuild/core'
import { pluginReact } from '@rsbuild/plugin-react'
import { tanstackRouter } from '@tanstack/router-plugin/rspack'

export default defineConfig({
  plugins: [pluginReact()],
  html: {
    favicon: "src/assets/favicon.ico",
    title: "Loco SaaS Starter",
  },
  server: {
    proxy: {
      "/api": {
        target: "http://127.0.0.1:5150",
        changeOrigin: true,
        secure: false,
      },
    },
  },
  tools: {
    rspack: {
      plugins: [
        tanstackRouter({
          target: 'react',
          autoCodeSplitting: true,
        }),
      ],
    },
  },
});
