import {defineConfig} from 'vite';
import {viteSingleFile} from 'vite-plugin-singlefile';

export default defineConfig({
  plugins: [viteSingleFile()],
  build: {
    outDir: 'bundle',
    target: 'esnext',
    minify: true,
    cssCodeSplit: false,
    assetsInlineLimit: 100_000_000,
    emptyOutDir: true,
    rollupOptions: {
      input: './runner.html',
      output: {entryFileNames: 'runner.js'},
    },
  },
});
