const w = new Worker(
  new URL("./workers/worker_event_handlers.js", import.meta.url).href,
  { type: "module" },
);
w.postMessage({});
