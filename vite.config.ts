import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';
import pkg from './package.json';

/**
 * 把 package.json 的 version 注入 HTML。
 *
 * index.html 是静态文件，读不到 package.json，也拿不到 Rust 的 CARGO_PKG_VERSION，
 * 因此曾是清单之外的一处手动版本源（长期停留在 v5.1.2）。此处用 transformIndexHtml
 * 在构建/开发时替换 %APP_VERSION%，使 index.html 不再持有版本号本身。
 */
function appVersionPlugin(): Plugin {
  return {
    name: 'patheditor-app-version',
    transformIndexHtml(html) {
      return html.replace(/%APP_VERSION%/g, pkg.version);
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), appVersionPlugin()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
});
