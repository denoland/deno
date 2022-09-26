const worker = new Worker(
  import.meta.resolve("./message_handler_error.ts"),
  { type: "module", name: "foo" },
);
worker.onmessage = () => {
  worker.postMessage("ready");
};
setTimeout(() => worker.terminate(), 30000);
