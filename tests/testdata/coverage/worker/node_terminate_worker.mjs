import { parentPort } from "node:worker_threads";

function coveredByWorker() {
  return "ready";
}

parentPort.postMessage(coveredByWorker());
setInterval(() => {}, 5_000);
