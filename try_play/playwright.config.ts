import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  webServer: {
    command: "../target/debug/deno task dev",
    port: 3000,
    reuseExistingServer: false,
    env: {
      DENO_COVERAGE_DIR: "./cov/",
    },
    gracefulShutdown: {
      signal: "SIGTERM",
      timeout: 5000,
    },
  },
  use: {
    baseURL: "http://localhost:3000",
  },
});
