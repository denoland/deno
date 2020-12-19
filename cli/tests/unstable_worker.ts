const w = new Worker(
  new URL("subdir/worker_unstable.ts", import.meta.url).href,
  {
    type: "module",
    deno: true,
    name: "Unstable Worker",
  },
);

w.postMessage({});
