import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  envDir: process.env.APP_ENV === 'test' ? false : undefined,
  base: '/admin/',
  plugins: [vue()],
  server: { host: '127.0.0.1', port: 13001 },
  build: { outDir: 'dist', emptyOutDir: true },
});
