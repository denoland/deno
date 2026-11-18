// Copyright 2018-2026 the Deno authors. MIT license.

import { parentPort, workerData } from "node:worker_threads";

function startNativeSleep(library: string) {
  const dylib = Deno.dlopen(library, {
    sleep_blocking: { parameters: ["u64"], result: "void", nonblocking: true },
  });
  // Still running in the worker's blocking pool when the worker goes away.
  void dylib.symbols.sleep_blocking(10000n);
}

if (parentPort) {
  startNativeSleep(workerData.library);
  process.exit(0);
} else {
  onmessage = async ({ data: { library, mode } }) => {
    if (mode === "close") {
      startNativeSleep(library);
      postMessage("started");
      close();
    } else {
      await navigator.locks.request("native-call", () => {
        startNativeSleep(library);
        postMessage("started");
        return new Promise(() => {});
      });
    }
  };
}
