// Benchmark measures time it takes to start and stop a number of workers.
const workerCount = 50;

async function bench(): Promise<void> {
  const workers: Worker[] = [];
  for (var i = 1; i <= workerCount; ++i) {
    const worker = new Worker("tests/subdir/bench_worker.ts");
    const promise = new Promise(
      (resolve): void => {
        worker.onmessage = (e): void => {
          if (e.data.cmdId === 0) resolve();
        };
      }
    );
    worker.postMessage({ cmdId: 0, action: 2 });
    await promise;
    workers.push(worker);
  }
  console.log("Done creating workers closing workers!");
  for (const worker of workers) {
    worker.postMessage({ action: 3 });
    await worker.closed; // Required to avoid a cmdId not in table error.
  }
  console.log("Finished!");
}

bench();
