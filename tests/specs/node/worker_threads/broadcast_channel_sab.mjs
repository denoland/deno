// Regression test for https://github.com/denoland/deno/issues/34836
// A SharedArrayBuffer posted over a BroadcastChannel must be deserializable by
// every worker that receives it, not just the first one.
import { BroadcastChannel, isMainThread, Worker } from "node:worker_threads";

const WORKERS = 3;

if (isMainThread) {
  const sharedBuffer = new SharedArrayBuffer(4);
  new Uint32Array(sharedBuffer)[0] = 12345;

  const mainChannel = new BroadcastChannel("shared-array-buffer");
  let done = 0;
  mainChannel.onmessage = (event) => {
    if (event.data === "request-buffer") {
      mainChannel.postMessage(sharedBuffer);
    } else if (event.data === "ok") {
      if (++done === WORKERS) {
        mainChannel.close();
      }
    }
  };

  for (let i = 0; i < WORKERS; i++) {
    new Worker(new URL(import.meta.url));
  }
} else {
  const workerChannel = new BroadcastChannel("shared-array-buffer");
  workerChannel.onmessage = (event) => {
    if (event.data instanceof SharedArrayBuffer) {
      console.log("SharedArrayBuffer bytes:", new Uint32Array(event.data)[0]);
      workerChannel.postMessage("ok");
      workerChannel.close();
    }
  };
  workerChannel.postMessage("request-buffer");
}
