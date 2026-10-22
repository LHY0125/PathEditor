import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src/gui/web/src'),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/gui/web/tests/setup.ts'],
    exclude: ['src/gui/web/e2e/**', 'node_modules/**', 'src/gui/src-tauri/**', '.claude/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'cobertura'],
      include: ['src/gui/web/src/core/**', 'src/gui/web/src/store/**', 'src/gui/web/src/hooks/**'],
      exclude: ['src/gui/web/src/main.tsx', 'src/gui/web/src/vite-env.d.ts'],
      thresholds: {
        lines: 80,
      },
    },
  },
});
