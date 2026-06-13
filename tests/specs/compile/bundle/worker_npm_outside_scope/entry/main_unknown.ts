// Entrypoint for the negative case. Spawns a worker whose bare npm import is
// not present in the build's snapshot; compiling this must error.
const worker = new Worker(
  new URL("../outside/worker_unknown.ts", import.meta.url),
  { type: "module" },
);
worker.postMessage("ping");
