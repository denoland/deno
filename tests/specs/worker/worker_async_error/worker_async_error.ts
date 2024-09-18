const worker = new Worker(
  import.meta.resolve("./async_error.ts"),
  { type: "module", name: "foo" },
);
setTimeout(() => worker.terminate(), 30000);
