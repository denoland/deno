// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  getEnvironmentData,
  isMainThread,
  parentPort,
  threadId,
  workerData,
} from "node:worker_threads";
import { once } from "node:events";

async function message(expectedMessage) {
  const [message] = await once(parentPort, "message");
  if (message !== expectedMessage) {
    console.log(`Expected the message "${expectedMessage}", but got`, message);
    // fail test
    parentPort.close();
  }
}

await message("Hello, how are you my thread?");

parentPort.postMessage("I'm fine!");

await new Promise((resolve) => setTimeout(resolve, 100));

parentPort.postMessage({
  isMainThread,
  threadId,
  workerData: Array.isArray(workerData) &&
      workerData[workerData.length - 1] instanceof MessagePort
    ? workerData.slice(0, -1)
    : workerData,
  envData: [getEnvironmentData("test"), getEnvironmentData(1)],
});
