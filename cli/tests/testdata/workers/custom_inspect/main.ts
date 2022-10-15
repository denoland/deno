new Worker(
  import.meta.resolve("./worker.ts"),
  { type: "module" },
);
