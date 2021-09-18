new Worker(
  new URL("084_worker_custom_inspect_worker.ts", import.meta.url).href,
  { type: "module" },
);
