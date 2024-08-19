import {
  isMainThread,
  MessageChannel,
  parentPort,
  Worker,
  workerData,
} from "node:worker_threads";

const useParentPort = Deno.env.get("PARENT_PORT") === "1";

if (useParentPort) {
  if (isMainThread) {
    const worker = new Worker(import.meta.filename);
    worker.postMessage("main says hi!");
    worker.on("message", (msg) => console.log(msg));
  } else {
    parentPort.on("message", (msg) => {
      console.log(msg);
      parentPort.postMessage("worker says hi!");
      parentPort.unref();
    });
  }
} else {
  if (isMainThread) {
    const { port1, port2 } = new MessageChannel();
    const worker = new Worker(import.meta.filename, {
      workerData: port2,
      transferList: [port2],
    });
    port1.postMessage("main says hi!");
    port1.on("message", (msg) => console.log(msg));
  } else {
    const port = workerData;
    port.on("message", (msg) => {
      console.log(msg);
      port.postMessage("worker says hi!");
      port.unref();
    });
  }
}
