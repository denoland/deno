// Regression test for https://github.com/denoland/deno/issues/14786
// Atomics.waitAsync in a worker requires V8 to post a delayed foreground
// task when the wait is resolved. Without the custom platform waking the
// event loop, the worker hangs forever.
Deno.test(async function atomicsWaitAsyncNotify() {
  const ia = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
  );

  const p = Promise.allSettled(
    Array(2).fill(0).map((_, i) => {
      const w = new Worker(
        new URL("./worker.ts", import.meta.url),
        {
          type: "module",
          name: `worker-${i}`,
        },
      );
      return new Promise<void>((res, rej) => {
        w.onmessage = (ev) => {
          if (ev.data.ok) res(ev.data);
          else rej(ev.data);
        };
        setTimeout(w.postMessage.bind(w), undefined, ia);
      }).finally(w.terminate.bind(w));
    }),
  );

  const results = await p;
  for (const r of results) {
    if (r.status === "rejected") throw r.reason;
  }
});
