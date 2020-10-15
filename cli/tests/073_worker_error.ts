const worker = new Worker(
  new URL("subdir/worker_error.ts", import.meta.url).href,
  { type: "module", name: "bar" },
);
setTimeout(() => worker.terminate(), 30000);
