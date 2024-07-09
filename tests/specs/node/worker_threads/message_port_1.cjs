const { parentPort, workerData } = require("worker_threads");

parentPort.on("message", (msg) => {
  const workerPort = workerData;
  parentPort.postMessage("Hello from worker on parentPort!");
  workerPort.postMessage("Hello from worker on workerPort!");
  workerPort.on("close", () => console.log("worker port closed"));
  workerPort.close();
});
