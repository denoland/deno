// Benchmark measures time it takes to start and stop a number of workers.
const workerCount = 50;

async function bench(): Promise<void> {
  const workers: Worker[] = [];
  for (let i = 1; i <= workerCount; ++i) {
    const worker = new Worker(
      new URL("workers/bench_worker.ts", import.meta.url).href,
      { type: "module" },
    );
    const promise = new Promise<void>((resolve): void => {
      worker.onmessage = (e): void => {
        if (e.data.cmdId === 0) resolve();
      };
    });
    worker.postMessage({ cmdId: 0, action: 2 });
    await promise;
    workers.push(worker);
  }
  console.log("Done creating workers closing workers!");
  for (const worker of workers) {
    const promise = new Promise<void>((resolve): void => {
      worker.onmessage = (e): void => {
        if (e.data.cmdId === 3) resolve();
      };
    });
    worker.postMessage({ action: 3 });
    await promise;
  }
  console.log("Finished!");
}

bench();
