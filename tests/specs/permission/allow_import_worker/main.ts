new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: {
    permissions: {
      import: ["127.0.0.1:4250"],
    },
  },
});
