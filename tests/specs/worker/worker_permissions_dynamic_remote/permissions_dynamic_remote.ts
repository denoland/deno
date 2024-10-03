new Worker(
  "http://localhost:4545/workers/dynamic_remote.ts",
  {
    type: "module",
    deno: {
      permissions: {
        // dynamic_remote.ts will import from example.com
        import: false,
      },
    },
  },
);
