import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { paraglideVitePlugin } from '@inlang/paraglide-js';
import type { Plugin } from 'vite';

const FROZEN_UNSAFE = /[\\/]node_modules[\\/]\.pnpm[\\/]nearley@/;
const PROTO_KEYS = 'constructor|valueOf|toString|toLocaleString|hasOwnProperty|isPrototypeOf|propertyIsEnumerable';
const ASSIGN = new RegExp(`(^|[^\\w$.])([A-Za-z_$][\\w$]*(?:\\.[A-Za-z_$][\\w$]*)*)\\.(${PROTO_KEYS})\\s*=(?!=)`, 'g');

const frozenPrototypeSafe = (): Plugin => ({
  name: 'nd-frozen-prototype-safe',
  transform(code, id) {
    if (!FROZEN_UNSAFE.test(id)) return null;
    return code.replace(ASSIGN, (_, pre, obj, key) => `${pre}${obj}[(Object.defineProperty(${obj},"${key}",{value:void 0,writable:!0,configurable:!0}),"${key}")]=`);
  },
});

export default defineConfig({
  plugins: [paraglideVitePlugin({ project: './project.inlang', outdir: './src/paraglide', strategy: ['baseLocale'] }), svelte(), frozenPrototypeSafe()],
  optimizeDeps: { rolldownOptions: { plugins: [frozenPrototypeSafe()] } },
  clearScreen: false,
  server: { port: 5183, strictPort: true },
  preview: { port: 5184, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    outDir: 'dist',
    target: 'es2022',
    emptyOutDir: true,
    assetsInlineLimit: (file: string) => (/\.(woff2?|ttf|otf|wav)$/i.test(file) ? false : undefined),
  },
});
