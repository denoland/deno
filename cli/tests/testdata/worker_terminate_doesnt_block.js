const WORKER_CODE = "while (true) {}";

const worker = new Worker(
  `data:text/javascript,${WORKER_CODE}`,
  { type: "module" },
);

const before = Date.now();
worker.terminate();
const after = Date.now();

// Terminating a worker that's stuck in an infinite loop will take 2 seconds.
// Let's take .5 seconds as definitely indicating a bug.
if (after - before >= 500) {
  console.log(
    `Calling worker.terminate() blocked for ${
      (after - before) / 1000
    } seconds.`,
  );
}
