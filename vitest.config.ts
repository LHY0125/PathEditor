import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
  test: {
    environment: 'jsdom',
    exclude: ['e2e/**', 'node_modules/**', 'gui/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'cobertura'],
      include: ['src/core/**', 'src/store/**', 'src/hooks/**'],
      exclude: ['src/main.tsx', 'src/vite-env.d.ts'],
      thresholds: {
        lines: 80,
      },
    },
  },
});
