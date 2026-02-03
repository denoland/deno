// Copyright 2018-2026 the Deno authors. MIT license.
// Test: Timing attack with fd reuse after worker termination

import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import fs from "node:fs";
import path from "node:path";

if (isMainThread) {
  const dir = import.meta.dirname!;
  const secretFile = path.join(dir, "secret.txt");
  const publicFile = path.join(dir, "public.txt");

  Deno.writeTextFileSync(secretFile, "SECRET DATA!");
  Deno.writeTextFileSync(publicFile, "public");

  console.log("=== Timing attack: worker termination + fd reuse ===\n");

  const worker = new Worker(import.meta.filename, {
    workerData: { publicFile }
  });

  worker.on("message", async (fd: number) => {
    console.log(`1. Worker opened file and sent fd: ${fd}`);

    await worker.terminate();
    console.log("2. Worker terminated (its fd is now closed)");

    // Now open secret file - kernel might reuse fd 3
    const secretFd = fs.openSync(secretFile, "r");
    console.log(`3. Opened secret file, got fd: ${secretFd}`);

    // Try to read using the OLD fd number from terminated worker
    console.log(`4. Trying to read using old fd ${fd}...`);
    try {
      const buf = Buffer.alloc(100);
      const bytesRead = fs.readSync(fd, buf, 0, 100, 0);
      const content = buf.toString("utf8", 0, bytesRead).trim();
      console.log(`5. Read succeeded: "${content}"`);

      if (content.includes("SECRET")) {
        console.log("\n!!! ATTACK SUCCESSFUL !!!");
      }
    } catch (e) {
      console.log(`5. Read failed: ${e.message}`);
    }

    fs.closeSync(secretFd);
    Deno.removeSync(secretFile);
    Deno.removeSync(publicFile);
  });
} else {
  const fd = fs.openSync(workerData.publicFile, "r");
  console.log(`   Worker: opened file, got fd ${fd}`);
  parentPort!.postMessage(fd);
}
