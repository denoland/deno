// Caps the worker's heap at `--memoryMb` MB. The worker allocates ~8 MB, so a
// small limit terminates it with ERR_WORKER_OUT_OF_MEMORY, a larger one lets
// it finish, and without `--unstable-worker-options` the limit is ignored.
const idx = Deno.args.indexOf("--memoryMb");
const memoryMb = idx >= 0 ? Number(Deno.args[idx + 1]) : undefined;

const worker = new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: memoryMb !== undefined ? { memoryMb } : undefined,
});

worker.onmessage = (e) => {
  console.log(e.data);
  worker.terminate();
  Deno.exit(0);
};

worker.onerror = (e) => {
  e.preventDefault();
  console.log(e.message);
  Deno.exit(0);
};
