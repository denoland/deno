// Regression test for https://github.com/denoland/deno/issues/14786
// Atomics.waitAsync in a worker requires V8 to post a foreground task
// to resolve the promise. Without the custom platform waking the event
// loop, the worker hangs forever.
Deno.test("Atomics.waitAsync resolves in worker", async () => {
  const sab = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT);
  const ia = new Int32Array(sab);

  const w = new Worker(new URL("./worker.ts", import.meta.url), {
    type: "module",
  });

  await new Promise<void>((resolve, reject) => {
    w.onmessage = (ev) => {
      if (ev.data === "waiting") {
        // Worker called waitAsync and is blocked — notify it.
        Atomics.notify(ia, 0);
      } else if (ev.data.ok) {
        resolve();
      } else {
        reject(new Error(ev.data.err));
      }
    };
    w.onerror = (e) => reject(e);
    w.postMessage(ia);
  });

  w.terminate();
});
