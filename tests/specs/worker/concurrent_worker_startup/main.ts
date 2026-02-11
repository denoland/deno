const WORKER_COUNT = 1000;
const TIMEOUT_MS = 60_000;

const workers: Worker[] = [];
const results = new Set<number>();

const done = new Promise<void>((resolve, reject) => {
  const timeout = setTimeout(() => {
    const failed: number[] = [];
    for (let i = 0; i < WORKER_COUNT; i++) {
      if (!results.has(i)) {
        failed.push(i);
      }
    }
    reject(
      new Error(
        `Timeout: ${failed.length}/${WORKER_COUNT} workers did not respond: [${
          failed.join(", ")
        }]`,
      ),
    );
  }, TIMEOUT_MS);

  for (let i = 0; i < WORKER_COUNT; i++) {
    const worker = new Worker(new URL("./worker.ts", import.meta.url).href, {
      type: "module",
    });
    workers.push(worker);

    worker.onmessage = (e) => {
      if (e.data === "ready") {
        results.add(i);
        if (results.size === WORKER_COUNT) {
          clearTimeout(timeout);
          resolve();
        }
      }
    };

    worker.onerror = (e) => {
      e.preventDefault();
      clearTimeout(timeout);
      reject(new Error(`Worker ${i} error: ${e.message}`));
    };
  }
});

try {
  await done;
  console.log(`All ${WORKER_COUNT} workers started successfully`);
} finally {
  for (const worker of workers) {
    worker.terminate();
  }
}
