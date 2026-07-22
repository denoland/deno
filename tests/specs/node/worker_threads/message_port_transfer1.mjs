import { parentPort } from "node:worker_threads";

parentPort.on("message", (message) => {
  const transferredPort = message;
  transferredPort.on("message", (message) => {
    console.log("Received message from main thread:", message);
    parentPort.postMessage("Reply from worker");
  });
  console.log("Worker thread started!");
});
