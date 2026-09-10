import { Worker } from "node:worker_threads";

const worker = new Worker(
  `require("node:worker_threads").parentPort.unref();`,
  { eval: true, stdin: true },
);

worker.on("exit", (code) => console.log(`exit ${code}`));
