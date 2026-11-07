import { defineConfig } from 'vite';

// One config, two pages: `vite build` -> landing page in dist/, `vite build --mode ui` -> tray flyout in ui-dist/.
export default defineConfig(({ mode }) => {
  const ui = mode === 'ui';
  return {
    root: ui ? 'ui' : 'site',
    base: './',
    build: { outDir: ui ? '../ui-dist' : '../dist', emptyOutDir: true },
  };
});
