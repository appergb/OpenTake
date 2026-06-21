import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri dev server expects a fixed port; harmless for plain web build.
  server: {
    port: 1420,
    strictPort: true,
  },
  clearScreen: false,
});
