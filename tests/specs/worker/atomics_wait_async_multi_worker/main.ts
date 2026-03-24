// Test that foreground tasks are delivered to the correct isolate when
// multiple workers are using Atomics.waitAsync concurrently.
Deno.test("multiple workers waitAsync resolve independently", async () => {
  const NUM_WORKERS = 4;
  const sab = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT);
  const ia = new Int32Array(sab);

  const workers: Worker[] = [];
  const results: Promise<void>[] = [];
  let waitingCount = 0;

  for (let i = 0; i < NUM_WORKERS; i++) {
    const w = new Worker(new URL("./worker.ts", import.meta.url), {
      type: "module",
    });
    workers.push(w);

    results.push(
      new Promise<void>((resolve, reject) => {
        w.onmessage = (ev) => {
          if (ev.data === "waiting") {
            waitingCount++;
            // Once all workers are waiting, notify them all.
            if (waitingCount === NUM_WORKERS) {
              Atomics.notify(ia, 0);
            }
          } else if (ev.data.ok) {
            resolve();
          } else {
            reject(new Error(`Worker ${i}: ${ev.data.err}`));
          }
        };
        w.onerror = (e) => reject(e);
        w.postMessage(ia);
      }),
    );
  }

  await Promise.all(results);

  for (const w of workers) {
    w.terminate();
  }
});
