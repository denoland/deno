// The parent holds full `--allow-import`, but spawns a worker that is
// explicitly restricted with `import: false`. The worker is loaded from a
// local file and statically imports a remote module. Statically analyzable
// imports in a worker must be checked against the worker's own (restricted)
// permissions, not the parent's, so the remote import is denied.
new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: {
    permissions: {
      import: false,
    },
  },
});
