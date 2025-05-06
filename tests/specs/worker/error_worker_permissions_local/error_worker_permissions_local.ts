new Worker(
  import.meta.resolve("../subdir/worker_types.ts"),
  { type: "module" },
);
