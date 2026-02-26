// Spawns a Deno server with DENO_COVERAGE_DIR set, waits for it to be
// ready, sends SIGTERM, then verifies coverage files were written.

const covDir = Deno.cwd() + "/cov_output";

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "--allow-net", "server.ts"],
  env: {
    DENO_COVERAGE_DIR: covDir,
  },
  stdout: "piped",
  stderr: "piped",
}).spawn();

// Wait for the server to be ready by reading stderr for the "Listening" message.
const decoder = new TextDecoder();
let stderr = "";
const reader = child.stderr.getReader();
while (true) {
  const { value, done } = await reader.read();
  if (done) break;
  stderr += decoder.decode(value);
  if (stderr.includes("Listening")) break;
}
reader.releaseLock();

// Send SIGTERM to the server process.
child.kill("SIGTERM");
const status = await child.status;

// The process should have been killed by SIGTERM.
console.log("signal:", status.signal);

// Check that coverage files were written.
let covFiles: string[] = [];
try {
  for await (const entry of Deno.readDir(covDir)) {
    if (entry.name.endsWith(".json")) {
      covFiles.push(entry.name);
    }
  }
} catch {
  // directory doesn't exist
}

if (covFiles.length > 0) {
  console.log("coverage files written:", covFiles.length);
} else {
  console.log("ERROR: no coverage files found");
}
