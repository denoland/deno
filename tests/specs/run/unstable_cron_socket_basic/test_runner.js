// Test runner that coordinates server and client
import { join } from "@std/path";

const testDir = new URL(".", import.meta.url).pathname;
const socketPath = join(testDir, "test.sock");

// Pre-cleanup: Remove stale socket file
try {
  Deno.removeSync(socketPath);
} catch {
  // Ignore if doesn't exist
}

// Start the server in background
const serverEnv = { ...Deno.env.toObject(), TEST_SOCKET_PATH: socketPath };
const serverProcess = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--allow-read",
    "--allow-write",
    "--allow-env",
    "--unstable-cron",
    join(testDir, "server.js"),
  ],
  env: serverEnv,
  stderr: "inherit",
}).spawn();

// Wait for server to start
await new Promise((resolve) => setTimeout(resolve, 500));

// Run the main script
const mainEnv = {
  ...Deno.env.toObject(),
  DENO_UNSTABLE_CRON_SOCK: `unix:${socketPath}`,
};
const mainProcess = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--unstable-cron",
    join(testDir, "main.js"),
  ],
  env: mainEnv,
  stdout: "inherit",
  stderr: "inherit",
}).spawn();

try {
  const [serverResult, mainResult] = await Promise.all([
    serverProcess.status,
    mainProcess.status,
  ]);

  console.error("Server exit code:", serverResult.code);
  console.error("Main exit code:", mainResult.code);

  // Exit with main script's exit code
  if (mainResult.code !== 0) {
    Deno.exit(mainResult.code);
  }
} finally {
  // Post-cleanup: Remove socket file
  try {
    Deno.removeSync(socketPath);
  } catch {
    // Ignore if already removed
  }
}
