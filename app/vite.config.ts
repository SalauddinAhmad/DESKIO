import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built assets from a file:// origin, so every path must be
// relative. The dev server port is fixed because tauri.conf.json points at it.
export default defineConfig({
  plugins: [react()],
  base: "./",
  clearScreen: false,
  server: { port: 5273, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true, target: "safari15" },
});
