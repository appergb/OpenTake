import {defineConfig} from 'vite';
import motionCanvasModule from '@motion-canvas/vite-plugin';

const motionCanvas =
  typeof motionCanvasModule === 'function'
    ? motionCanvasModule
    : (motionCanvasModule as unknown as {default: typeof motionCanvasModule}).default;

export default defineConfig({
  plugins: [
    motionCanvas({
      project: './src/project.ts',
      editor: '@opentake/motion-canvas-runner',
    }),
    {
      name: 'opentake:motion-canvas-vite7-target',
      enforce: 'post',
      config: () => ({build: {target: 'esnext'}}),
    },
  ],
  build: {
    outDir: '.motion-build',
    target: 'esnext',
    minify: true,
    rollupOptions: {
      external: id => id.startsWith('@motion-canvas/'),
      output: {entryFileNames: 'project.js'},
    },
  },
});
