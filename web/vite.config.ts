import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  // The Rust service embeds this output, so entry and asset names stay fixed.
  build: {
    outDir: '../src/web/dist',
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      output: { entryFileNames: 'app.js', assetFileNames: 'app.[ext]', chunkFileNames: 'chunk-[hash].js' },
    },
  },
  // `npm run dev` serves the UI with hot reload while the Rust service keeps
  // owning the API, so no CORS or duplicated logic in development.
  server: {
    port: 5173,
    proxy: { '/api': 'http://127.0.0.1:8080', '/feeds': 'http://127.0.0.1:8080', '/fonts': 'http://127.0.0.1:8080' },
  },
});
