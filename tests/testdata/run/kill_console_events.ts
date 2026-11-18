// Sends SIGBREAK and SIGINT to every process attached to the current
// console (pid 0) and asserts that this process receives both of them.
// The test runner spawns this script with CREATE_NEW_CONSOLE so the
// events do not propagate to other processes.
const received = new Set<string>();
const done = Promise.withResolvers<void>();

for (const signal of ["SIGBREAK", "SIGINT"] as const) {
  Deno.addSignalListener(signal, () => {
    received.add(signal);
    if (received.size === 2) {
      done.resolve();
    }
  });
}

Deno.kill(0, "SIGBREAK");
Deno.kill(0, "SIGINT");

const timeout = setTimeout(() => {
  console.log(`timed out, received: ${[...received].join(",")}`);
  Deno.exit(1);
}, 30_000);

await done.promise;
clearTimeout(timeout);
console.log("received SIGBREAK and SIGINT");
