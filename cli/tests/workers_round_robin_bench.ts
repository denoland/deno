// Benchmark measures time it takes to send a message to a group of workers one
// at a time and wait for a response from all of them. Just a general
// throughput and consistency benchmark.
const data = "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n";
const workerCount = 4;
const cmdsPerWorker = 400;

export interface ResolvableMethods<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}

export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;

export function createResolvable<T>(): Resolvable<T> {
  let methods: ResolvableMethods<T>;
  const promise = new Promise<T>((resolve, reject): void => {
    methods = { resolve, reject };
  });
  // TypeScript doesn't know that the Promise callback occurs synchronously
  // therefore use of not null assertion (`!`)
  return Object.assign(promise, methods!) as Resolvable<T>;
}

function handleAsyncMsgFromWorker(
  promiseTable: Map<number, Resolvable<string>>,
  msg: { cmdId: number; data: string },
): void {
  const promise = promiseTable.get(msg.cmdId);
  if (promise === null) {
    throw new Error(`Failed to find promise: cmdId: ${msg.cmdId}, msg: ${msg}`);
  }
  promise?.resolve(data);
}

async function main(): Promise<void> {
  const workers: Array<[Map<number, Resolvable<string>>, Worker]> = [];
  for (let i = 1; i <= workerCount; ++i) {
    const worker = new Worker(
      new URL("subdir/bench_worker.ts", import.meta.url).href,
      { type: "module" },
    );
    const promise = createResolvable<void>();
    worker.onmessage = (e): void => {
      if (e.data.cmdId === 0) promise.resolve();
    };
    worker.postMessage({ cmdId: 0, action: 2 });
    await promise;
    workers.push([new Map(), worker]);
  }
  // assign callback function
  for (const [promiseTable, worker] of workers) {
    worker.onmessage = (e): void => {
      handleAsyncMsgFromWorker(promiseTable, e.data);
    };
  }
  for (const cmdId of Array(cmdsPerWorker).keys()) {
    const promises: Array<Promise<string>> = [];
    for (const [promiseTable, worker] of workers) {
      const promise = createResolvable<string>();
      promiseTable.set(cmdId, promise);
      worker.postMessage({ cmdId: cmdId, action: 1, data });
      promises.push(promise);
    }
    for (const promise of promises) {
      await promise;
    }
  }
  for (const [, worker] of workers) {
    const promise = createResolvable<void>();
    worker.onmessage = (e): void => {
      if (e.data.cmdId === 3) promise.resolve();
    };
    worker.postMessage({ action: 3 });
    await promise;
  }
  console.log("Finished!");
}

main();
