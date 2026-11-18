// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// A worker that leaves a long native call running must still release what
// its host can observe (its channels, exit event, locks) without waiting for
// that call to return.

import { Worker as NodeWorker } from "node:worker_threads";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [prefix, suffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const library = `${targetDir}/${prefix}test_ffi.${suffix}`;
const workerUrl = new URL(
  "./worker_release_during_native_call_worker.ts",
  import.meta.url,
);
// The worker's native call sleeps for 10000ms; leave room for slow CI.
const PROMPT_MS = 5000;

function report(name: string, start: number) {
  const elapsed = performance.now() - start;
  console.log(`${name}: ${elapsed < PROMPT_MS ? "prompt" : "delayed"}`);
}

async function startWebWorker(mode: string) {
  const worker = new Worker(workerUrl.href, { type: "module" });
  const started = new Promise<void>((resolve) => {
    worker.onmessage = () => resolve();
  });
  worker.postMessage({ library, mode });
  await started;
  return worker;
}

if (Deno.args[0] === "close-child") {
  // The process can only exit once the closed worker's channels are released.
  await startWebWorker("close");
} else {
  {
    const start = performance.now();
    const status = await new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--no-lock",
        "--allow-ffi",
        "--allow-read",
        "--quiet",
        import.meta.filename!,
        "close-child",
      ],
    }).output();
    if (!status.success) throw new Error("close child failed");
    // The child also pays process startup; the native call is 10000ms.
    const elapsed = performance.now() - start;
    console.log(`web close: ${elapsed < 8000 ? "prompt" : "delayed"}`);
  }

  {
    const worker = new NodeWorker(workerUrl, { workerData: { library } });
    const exited = new Promise<void>((resolve) => worker.on("exit", resolve));
    const start = performance.now();
    await exited;
    report("node process.exit", start);
  }

  {
    const worker = await startWebWorker("lock");
    worker.terminate();
    const start = performance.now();
    await navigator.locks.request("native-call", () => {});
    report("web terminate lock", start);
  }
}
