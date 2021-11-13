const worker = new Worker(
  new URL("message_handler_error.ts", import.meta.url).href,
  { type: "module", name: "foo" },
);
worker.onmessage = () => {
  worker.postMessage("ready");
};
setTimeout(() => worker.terminate(), 30000);
