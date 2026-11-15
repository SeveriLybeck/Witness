import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: { port: 9280, strictPort: true, proxy: { '/api': 'http://127.0.0.1:9260' } },
  test: {
    environment: 'jsdom',
    setupFiles: ['./test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}', 'test/**/*.test.{ts,tsx}'],
    maxWorkers: 2,
    css: { include: [/.+/] },
  },
});
