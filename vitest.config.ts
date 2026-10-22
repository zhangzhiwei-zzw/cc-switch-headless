import path from "node:path";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    include: ["{src,tests}/**/*.{test,spec}.{ts,tsx,js,jsx}"],
    exclude: ["tests/browser/**"],
    environment: "jsdom",
    setupFiles: ["./tests/setupGlobals.ts", "./tests/setupTests.ts"],
    globals: true,
    // Large DOM editors need bounded concurrency on high-core CI hosts.
    maxWorkers: 2,
    minWorkers: 1,
    testTimeout: 15_000,
    coverage: {
      reporter: ["text", "lcov"],
    },
  },
});
