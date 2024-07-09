import workerThreads from "node:worker_threads";

const { port1: mainPort, port2: workerPort } = new workerThreads
  .MessageChannel();

// Note: not using Promise.withResolver() because it's not available in Node.js
const deferred = createDeferred();

const worker = new workerThreads.Worker(
  import.meta.resolve("./message_port_1.cjs"),
  {
    workerData: workerPort,
    transferList: [workerPort],
  },
);

worker.on("message", (data) => {
  console.log("worker:", data);
  mainPort.on("message", (msg) => {
    console.log("mainPort:", msg);
    deferred.resolve();
  });
  mainPort.on("close", (_msg) => {
    console.log("mainPort closed");
  });
});

worker.postMessage("Hello from parent");
await deferred.promise;
await worker.terminate();
mainPort.close();

function createDeferred() {
  let resolveCallback;
  let rejectCallback;
  const promise = new Promise((resolve, reject) => {
    resolveCallback = resolve;
    rejectCallback = reject;
  });
  return { promise, resolve: resolveCallback, reject: rejectCallback };
}
