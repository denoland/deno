// Spawns `deno serve` and verifies that it shuts down cleanly (exit code 0)
// when it receives SIGTERM/SIGINT, instead of being terminated by the OS
// default signal handler (which would report the killing signal).

const serverScript = new URL("./server.ts", import.meta.url).pathname;

async function run(signal: Deno.Signal, port: number) {
  const child = new Deno.Command(Deno.execPath(), {
    args: ["serve", "--allow-net", "--port", String(port), serverScript],
    stdout: "null",
    stderr: "null",
  }).spawn();

  // Wait until the server is accepting connections. Once a request succeeds
  // the signal listeners have been registered, since they are added
  // synchronously right after `Deno.serve()` returns.
  let ready = false;
  for (let i = 0; i < 1000; i++) {
    try {
      const resp = await fetch(`http://localhost:${port}/`);
      await resp.text();
      ready = true;
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 10));
    }
  }
  if (!ready) {
    console.log(`${signal}: server never became ready`);
    child.kill("SIGKILL");
    await child.status;
    return;
  }

  child.kill(signal);
  const status = await child.status;
  console.log(`${signal}: code=${status.code} signal=${status.signal}`);
}

await run("SIGTERM", 23497);
await run("SIGINT", 23498);
