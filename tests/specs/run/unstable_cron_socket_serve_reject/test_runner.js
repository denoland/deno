import { join } from "@std/path";

const testDir = new URL(".", import.meta.url).pathname;
const tempDir = Deno.makeTempDirSync();
const cronSocketPath = join(tempDir, "cron.sock");
const controlSocketPath = join(tempDir, "control.sock");

// 1. Start cron socket listener (we are the server for cron)
const cronListener = Deno.listen({ transport: "unix", path: cronSocketPath });

// 2. Spawn main.js with both env vars set
const mainProcess = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--unstable-cron",
    "--allow-net",
    join(testDir, "main.js"),
  ],
  env: {
    DENO_UNSTABLE_CRON_SOCK: `unix:${cronSocketPath}`,
    DENO_UNSTABLE_CONTROL_SOCK: `unix:${controlSocketPath}`,
  },
}).spawn();

// Give Deno a moment to start and create its control socket
await new Promise((resolve) => setTimeout(resolve, 100));

// 3. Connect to control socket (Deno is the server, we are the client)
const controlConn = await Deno.connect({
  transport: "unix",
  path: controlSocketPath,
});

const controlReader = controlConn.readable
  .pipeThrough(new TextDecoderStream())
  .getReader();
const controlWriter = controlConn.writable.getWriter();

// Send Start command
const startCmd = JSON.stringify({
  cwd: Deno.cwd(),
  args: ["run", "--unstable-cron", "--allow-net", join(testDir, "main.js")],
  env: [],
}) + "\n";
await controlWriter.write(new TextEncoder().encode(startCmd));

// 4. Accept cron socket connection (Deno connects to us)
const cronConn = await cronListener.accept();
const cronReader = cronConn.readable
  .pipeThrough(new TextDecoderStream())
  .getReader();
const cronWriter = cronConn.writable.getWriter();

let buffer = "";
let receivedServing = false;
const registeredCrons = [];

// 5. Process messages concurrently
async function readCronMessages() {
  let receivedRejectAck = false;

  while (true) {
    const { value: chunk, done } = await cronReader.read();
    if (done) break;

    buffer += chunk;
    const lines = buffer.split("\n");
    buffer = lines.pop() || "";

    for (const line of lines) {
      if (!line.trim()) continue;

      const msg = JSON.parse(line);
      if (msg.kind === "register") {
        console.error(
          "[CRON SERVER] Registered:",
          msg.crons.map((c) => c.name).join(", "),
        );
        registeredCrons.push(...msg.crons.map((c) => c.name));

        // If we already received "Serving", this is an error
        if (receivedServing) {
          console.error(
            "[CRON SERVER] ERROR: Unexpected registration after serve",
          );
          Deno.exit(1);
        }
      } else if (msg.kind === "reject-ack") {
        console.error("[CRON SERVER] Received reject-ack");
        receivedRejectAck = true;
      }
    }
  }

  return receivedRejectAck;
}

async function readControlMessages() {
  while (true) {
    const { value: chunk, done } = await controlReader.read();
    if (done) break;

    const lines = chunk.split("\n");
    for (const line of lines) {
      if (!line.trim()) continue;

      const msg = JSON.parse(line);
      if (msg === "Serving") {
        receivedServing = true;
        console.error("[CONTROL CLIENT] Received Serving signal from Deno");

        // Send rejection on cron socket
        const rejection = JSON.stringify({
          kind: "reject-new-crons",
          reason: "No crons after serve",
        }) + "\n";
        await cronWriter.write(new TextEncoder().encode(rejection));
        console.error("[CRON SERVER] Sent rejection");

        // Wait to ensure no more registrations arrive
        await new Promise((resolve) => setTimeout(resolve, 2000));

        // Read side is closed by Deno. We cannot close the whole connection here because we are still reading
        // in the readCronMessages loop
        await cronConn.closeWrite();
        return;
      }
    }
  }
}

const [receivedRejectAck] = await Promise.all([
  readCronMessages(),
  readControlMessages(),
]);

if (!receivedRejectAck) {
  console.error("[CRON SERVER] ERROR: Did not receive reject-ack");
  Deno.exit(1);
}

console.error("[CRON SERVER] Registered crons:", registeredCrons.join(", "));
if (registeredCrons.length === 1 && registeredCrons[0] === "early-cron") {
  console.error("[CRON SERVER] SUCCESS: Only early cron was registered");
} else {
  console.error("[CRON SERVER] ERROR: Unexpected registrations");
  Deno.exit(1);
}

// Cleanup
try {
  controlConn.close();
} catch {
  // Already closed
}
cronListener.close();

mainProcess.kill();
