const worker = new Worker(
  new URL("async_error.ts", import.meta.url).href,
  { type: "module", name: "foo" },
);
setTimeout(() => worker.terminate(), 30000);
