import { parentPort, resourceLimits } from "node:worker_threads";

parentPort.postMessage(resourceLimits);
