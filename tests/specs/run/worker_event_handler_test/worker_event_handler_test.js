const w = new Worker(
  import.meta.resolve("./worker_event_handlers.js"),
  { type: "module" },
);
w.postMessage({});
