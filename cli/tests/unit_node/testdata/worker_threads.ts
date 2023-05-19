// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  getEnvironmentData,
  isMainThread,
  parentPort,
  threadId,
  workerData,
} from "node:worker_threads";
import { once } from "node:events";

async function message(expectedMessage: string) {
  const [message] = await once(parentPort as any, "message");
  if (message !== expectedMessage) {
    console.log(`Expected the message "${expectedMessage}", but got`, message);
    // fail test
    (parentPort as any).close();
  }
}

await message("Hello, how are you my thread?");

(parentPort as any).postMessage("I'm fine!");

await new Promise((resolve) => setTimeout(resolve, 100));

(parentPort as any).postMessage({
  isMainThread,
  threadId,
  workerData: Array.isArray(workerData) &&
      workerData[workerData.length - 1] instanceof MessagePort
    ? workerData.slice(0, -1)
    : workerData,
  envData: [getEnvironmentData("test"), getEnvironmentData(1)],
});
