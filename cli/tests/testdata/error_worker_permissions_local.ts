new Worker(
  import.meta.resolve("./subdeb/worker_types.ts"),
  { type: "module" },
);
