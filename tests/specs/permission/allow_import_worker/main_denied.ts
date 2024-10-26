new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: {
    permissions: {
      import: [],
    },
  },
});
