// Copyright 2018-2026 the Deno authors. MIT license.
import { isMainThread, parentPort, Worker } from "node:worker_threads";
import fs from "node:fs";
import path from "node:path";

if (isMainThread) {
  const testFile = path.join(import.meta.dirname, "main.mjs");
  const fd = fs.openSync(testFile, "r");
  console.log(`main: opened fd ${fd} (>= 3: ${fd >= 3})`);

  const w = new Worker(import.meta.filename);
  w.postMessage(fd);

  w.once("message", (msg) => {
    console.log(`main: worker result: ${msg}`);
    fs.closeSync(fd);
    console.log("main: closed fd");
    w.terminate();
  });
} else {
  parentPort.once("message", (fd) => {
    console.log(`worker: received fd ${fd}`);

    // Read from the fd received from the main thread
    const buf = Buffer.alloc(64);
    const bytesRead = fs.readSync(fd, buf, 0, 64, 0);
    console.log(`worker: read ${bytesRead} bytes`);
    console.log(
      `worker: starts with copyright: ${
        buf.toString().startsWith("// Copyright")
      }`,
    );

    parentPort.postMessage("ok");
  });
}
