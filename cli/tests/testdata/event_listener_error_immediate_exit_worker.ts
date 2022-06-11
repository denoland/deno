new Worker(
  new URL("event_listener_error_immediate_exit.ts", import.meta.url).href,
  { type: "module" },
);
