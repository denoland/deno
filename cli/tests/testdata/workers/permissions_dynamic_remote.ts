new Worker(
  "http://localhost:4545/workers/dynamic_remote.ts",
  {
    type: "module",
    deno: {
      permissions: {
        net: false,
      },
    },
  },
);
