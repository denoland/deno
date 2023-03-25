const w = new Worker(
  import.meta.resolve("../workers/worker_unstable.ts"),
  { type: "module", name: "Unstable Worker" },
);

w.postMessage({});
