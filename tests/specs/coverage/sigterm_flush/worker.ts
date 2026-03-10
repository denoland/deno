// Worker that does some work so it shows up in coverage.
function workerTask(n: number): number {
  if (n <= 1) return n;
  return workerTask(n - 1) + workerTask(n - 2);
}

const result = workerTask(10);
self.postMessage({ result });
