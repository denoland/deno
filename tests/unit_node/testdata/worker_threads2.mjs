// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { parentPort, workerData } from "node:worker_threads";
import { once } from "node:events";

async function message(expectedMessage) {
  const [message] = await once(parentPort, "message");
  if (message !== expectedMessage) {
    console.log(`Expected the message "${expectedMessage}", but got`, message);
    // fail test
    parentPort.close();
  }
}

const uint = new Uint8Array(workerData.sharedArrayBuffer);
uint[0]++;
await message("Hello");
parentPort.postMessage("Hello");
