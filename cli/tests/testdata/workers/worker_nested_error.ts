const worker = new Worker(
  new URL("worker_error.ts", import.meta.url).href,
  { type: "module", name: "baz" },
);
setTimeout(() => worker.terminate(), 30000);
