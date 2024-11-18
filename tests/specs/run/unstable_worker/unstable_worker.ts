const w = new Worker(
  import.meta.resolve("./worker_unstable.ts"),
  { type: "module", name: "Unstable Worker" },
);

w.postMessage({});
