import {
  isMainThread,
  MessageChannel,
  Worker,
  workerData,
} from "node:worker_threads";

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
