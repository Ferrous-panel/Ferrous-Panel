import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import { fileURLToPath } from 'url';
import { defineConfig } from 'vite';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, '.'),
    },
  },
  server: {
    hmr: false,
    ws: false,
    watch: null,
  },
  build: {
    chunkSizeWarningLimit: 2000,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/recharts') || id.includes('node_modules/d3-')) {
            return 'vendor-charts';
          }
          if (id.includes('node_modules/lucide-react')) {
            return 'vendor-icons';
          }
          if (id.includes('node_modules/react') || id.includes('node_modules/scheduler')) {
            return 'vendor-react';
          }
          if (id.includes('/src/features/websites/') || id.includes('/src/features/oneclick/')) {
            return 'feature-websites';
          }
          if (id.includes('/src/features/edge-cloud/') || id.includes('/src/features/rust-core/')) {
            return 'feature-rust-edge';
          }
          if (id.includes('/src/features/cpanel/') || id.includes('/src/features/security/') || id.includes('/src/features/optimizer/')) {
            return 'feature-ops';
          }
          if (id.includes('/src/features/files/') || id.includes('/src/features/ssh/') || id.includes('/src/features/databases/') || id.includes('/src/features/mail/') || id.includes('/src/features/php/')) {
            return 'feature-workspace';
          }
        },
      },
    },
  },
});
