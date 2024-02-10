const worker = new Worker(
  import.meta.resolve("./worker_error.ts"),
  { type: "module", name: "baz" },
);
setTimeout(() => worker.terminate(), 30000);
