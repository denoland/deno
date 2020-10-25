const w = new Worker(
  new URL("http://localhost:4545/cli/tests/subdir/worker_types.ts").toString(),
  { type: "module" },
);
w.terminate();
