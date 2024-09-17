const worker = new Worker(
  import.meta.resolve("./error.ts"),
  { type: "module", name: "bar" },
);
setTimeout(() => worker.terminate(), 30000);
