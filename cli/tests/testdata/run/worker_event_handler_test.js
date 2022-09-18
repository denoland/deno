const w = new Worker(
  import.meta.resolve("../workers/worker_event_handlers.js"),
  { type: "module" },
);
w.postMessage({});
