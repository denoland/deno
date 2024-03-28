import workerThreads from "node:worker_threads";

const { port1: mainPort, port2: workerPort } = new workerThreads
  .MessageChannel();

const deferred = createDeferred();

const worker = new workerThreads.Worker(
  "./worker.cjs",
  {
    workerData: { workerPort },
    transferList: [workerPort],
  },
);

worker.on("message", (data) => {
  console.log("worker:", data);
  //   assertEquals(data, "Hello from worker on parentPort!");
  mainPort.on("message", (msg) => {
    console.log("mainPort:", msg);
    // assertEquals(msg, "Hello from worker on workerPort!");
    deferred.resolve();
  });
  mainPort.on("close", (msg) => {
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
