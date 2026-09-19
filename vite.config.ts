import { defineConfig } from "vite";
import { resolve } from "path";

export default defineConfig({
  root: "src",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        taskbar: resolve(__dirname, "src/taskbar/index.html"),
        launcher: resolve(__dirname, "src/launcher/index.html"),
        setup: resolve(__dirname, "src/setup/index.html"),
        consent: resolve(__dirname, "src/consent/index.html"),
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
