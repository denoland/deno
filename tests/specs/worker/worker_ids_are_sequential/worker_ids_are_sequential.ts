import {
  isMainThread,
  parentPort,
  threadId,
  Worker,
} from "node:worker_threads";

console.log("threadId", threadId);

if (isMainThread) {
  const worker = new Worker(new URL(import.meta.url));
  worker.on("message", (msg) => console.log("from worker:", msg));
  worker.on("error", () => {
    throw new Error("error");
  });
  worker.on("exit", (code) => {
    if (code !== 0) {
      reject(new Error(`Worker stopped with exit code ${code}`));
    }
  });
} else if (threadId == 1) {
  const worker = new Worker(new URL(import.meta.url));
  worker.on("message", (msg) => console.log("from worker:", msg));
  worker.on("error", () => {
    throw new Error("error");
  });
  worker.on("exit", (code) => {
    if (code !== 0) {
      reject(new Error(`Worker stopped with exit code ${code}`));
    }
  });
} else {
  parentPort.postMessage("hello!");
}
