// The parent holds `--allow-read`, but spawns a worker that is explicitly
// restricted with `read: false`. The worker then tries to read a local file
// through a dynamic text import. That import must be checked against the
// worker's own (restricted) permissions, not the parent's, so it is denied.
const worker = new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: {
    permissions: {
      read: false,
    },
  },
});
