const w = new Worker(
  new URL("workers/worker_unstable.ts", import.meta.url).href,
  { type: "module", name: "Unstable Worker" },
);

w.postMessage({});
