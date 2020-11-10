const worker = new Worker(
  new URL("073_worker_error.ts", import.meta.url).href,
  { type: "module", name: "baz" },
);
setTimeout(() => worker.terminate(), 30000);
