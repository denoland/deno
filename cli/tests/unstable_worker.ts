const w = new Worker(
  new URL("workers/worker_unstable.ts", import.meta.url).href,
  {
    type: "module",
    //TODO(Soremwar)
    //Fix d.ts definition
    //deno-lint-ignore ban-ts-comment
    //@ts-ignore
    deno: {
      namespace: true,
    },
    name: "Unstable Worker",
  },
);

w.postMessage({});
