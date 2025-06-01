// Benchmark measures time it takes to send a message to a group of workers one
// at a time and wait for a response from all of them. Just a general
// throughput and consistency benchmark.
const data = "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n";
const workerCount = 4;
const cmdsPerWorker = 400;

function handleAsyncMsgFromWorker(
  promiseTable: Map<number, ReturnType<typeof Promise.withResolvers<string>>>,
  msg: { cmdId: number; data: string },
) {
  const promise = promiseTable.get(msg.cmdId);
  if (promise === null) {
    throw new Error(`Failed to find promise: cmdId: ${msg.cmdId}, msg: ${msg}`);
  }
  promise?.resolve(data);
}

async function main() {
  const workers: Array<
    [Map<number, ReturnType<typeof Promise.withResolvers<string>>>, Worker]
  > = [];
  for (let i = 1; i <= workerCount; ++i) {
    const worker = new Worker(
      import.meta.resolve("./bench_worker.ts"),
      { type: "module" },
    );
    const { promise, resolve } = Promise.withResolvers<void>();
    worker.onmessage = (e) => {
      if (e.data.cmdId === 0) resolve();
    };
    worker.postMessage({ cmdId: 0, action: 2 });
    await promise;
    workers.push([new Map(), worker]);
  }
  // assign callback function
  for (const [promiseTable, worker] of workers) {
    worker.onmessage = (e) => {
      handleAsyncMsgFromWorker(promiseTable, e.data);
    };
  }
  for (const cmdId of Array(cmdsPerWorker).keys()) {
    const promises: Array<Promise<string>> = [];
    for (const [promiseTable, worker] of workers) {
      const deferred = Promise.withResolvers<string>();
      promiseTable.set(cmdId, deferred);
      worker.postMessage({ cmdId: cmdId, action: 1, data });
      promises.push(deferred.promise);
    }
    for (const promise of promises) {
      await promise;
    }
  }
  for (const [, worker] of workers) {
    const { promise, resolve } = Promise.withResolvers<void>();
    worker.onmessage = (e) => {
      if (e.data.cmdId === 3) resolve();
    };
    worker.postMessage({ action: 3 });
    await promise;
  }
  console.log("Finished!");
}

main();
