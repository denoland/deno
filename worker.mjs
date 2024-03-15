import {
  isMainThread,
  MessageChannel,
  parentPort,
  receiveMessageOnPort,
  Worker,
  workerData,
} from "node:worker_threads";

if (isMainThread) {
  const { port1: mainPort, port2: workerPort } = new MessageChannel();

  // This re-loads the current file inside a Worker instance.
  const w = new Worker(import.meta.filename, {
    workerData: { workerPort },
    transferList: [workerPort],
  });

  w.on("message", (data) => {
    console.log("message from worker", data);

    const msg = receiveMessageOnPort(mainPort).message;
    console.log("message on mainPort", msg);
    w.terminate();
  });

  w.postMessage("Hello from parent");
} else {
  console.log("Inside Worker!");
  parentPort.on("message", (msg) => {
    console.log("message from main", msg);
    parentPort.postMessage("Hello from worker on parentPort!");
    workerData.workerPort.postMessage("Hello from worker on workerPort!");
  });
}
