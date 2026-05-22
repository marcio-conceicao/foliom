import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  // Force Svelte to resolve to its BROWSER entry inside happy-dom (the
  // default resolveConditions for vitest server-mode picks the SSR entry,
  // which throws "mount is not available on the server"). Listing the
  // browser conditions explicitly mirrors what vite-plugin-svelte uses
  // during a real `vite dev` run.
  resolve: {
    conditions: ['browser', 'svelte', 'development'],
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    include: ['src/**/*.test.ts'],
    server: {
      deps: {
        inline: ['svelte'],
      },
    },
  },
});
