new Worker(
  new URL("./subdeb/worker_types.ts", import.meta.url).toString(),
  { type: "module" },
);
