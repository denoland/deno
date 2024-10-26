import { isMainThread, parentPort, Worker } from "node:worker_threads";

function onMessageOneshot() {
  console.log("Got message from main thread!");
  parentPort.off("message", onMessageOneshot);
}

if (isMainThread) {
  // This re-loads the current file inside a Worker instance.
  const w = new Worker(import.meta.filename);

  setTimeout(() => {
    w.postMessage("Hello! I am from the main thread.");
  }, 500);
} else {
  console.log("Inside Worker!");
  console.log(isMainThread); // Prints 'false'.
  parentPort.on("message", onMessageOneshot);
}
