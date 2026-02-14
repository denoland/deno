import { join } from "@std/path";

const testDir = new URL(".", import.meta.url).pathname;
const tempDir = Deno.makeTempDirSync();
const cronSocketPath = join(tempDir, "cron.sock");
const controlSocketPath = join(tempDir, "control.sock");

// 1. Spawn main.js with NO DENO_UNSTABLE_CRON_SOCK initially
const mainProcess = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--unstable-cron",
    "--allow-net",
    join(testDir, "main.js"),
  ],
  env: {
    // NO DENO_UNSTABLE_CRON_SOCK here!
    DENO_UNSTABLE_CONTROL_SOCK: `unix:${controlSocketPath}`,
  },
  stderr: "inherit",
}).spawn();
console.error("[TEST] Main process spawned");

// Give Deno a moment to start and create its control socket
await new Promise((resolve) => setTimeout(resolve, 500));

// 2. Connect to control socket
console.error("[TEST] Connecting to control socket");
const controlConn = await Deno.connect({
  transport: "unix",
  path: controlSocketPath,
});
console.error("[TEST] Connected to control socket");

const controlWriter = controlConn.writable.getWriter();

// 3. Start cron socket listener NOW (right before sending Start message)
const cronListener = Deno.listen({ transport: "unix", path: cronSocketPath });
console.error("[TEST] Cron listener started");

// 4. Send Start command WITH DENO_UNSTABLE_CRON_SOCK in env
const startCmd = JSON.stringify({
  cwd: Deno.cwd(),
  args: ["run", "--unstable-cron", "--allow-net", join(testDir, "main.js")],
  env: [["DENO_UNSTABLE_CRON_SOCK", `unix:${cronSocketPath}`]],
}) + "\n";
console.error("[TEST] Sending Start command");
await controlWriter.write(new TextEncoder().encode(startCmd));
console.error("[TEST] Start command sent");

// 5. Accept cron socket connection
console.error("[TEST] Waiting for cron connection");
const cronConn = await cronListener.accept();
console.error("[TEST] Cron connection accepted");

const cronReader = cronConn.readable
  .pipeThrough(new TextDecoderStream())
  .getReader();

// 6. Read cron registration message
let buffer = "";

const timeout = setTimeout(() => {
  console.error("[TEST] ERROR: Timeout waiting for cron registration");
  cronConn.close();
  controlConn.close();
  cronListener.close();
  mainProcess.kill();
  Deno.exit(1);
}, 5000);

while (true) {
  const { value: chunk, done } = await cronReader.read();
  if (done) {
    console.error("[TEST] ERROR: Connection closed without registration");
    clearTimeout(timeout);
    cronConn.close();
    controlConn.close();
    cronListener.close();
    mainProcess.kill();
    Deno.exit(1);
  }

  buffer += chunk;
  const lines = buffer.split("\n");
  buffer = lines.pop() || "";

  for (const line of lines) {
    if (!line.trim()) continue;

    console.error(`[TEST] Received: ${line}`);
    const msg = JSON.parse(line);
    if (msg.kind === "register") {
      console.error(
        `[TEST] Registered crons: ${msg.crons.map((c) => c.name).join(", ")}`,
      );

      // Success! The cron connected to the socket from Start command
      console.error("[TEST] SUCCESS: Cron handler reloaded and connected");
      clearTimeout(timeout);
      cronConn.close();
      controlConn.close();
      cronListener.close();
      mainProcess.kill();
      Deno.exit(0);
    }
  }
}
