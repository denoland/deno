// Test for https://github.com/denoland/deno/issues/12658
//
// If a worker is terminated immediately after construction, and the worker's
// main module uses top-level await, V8 has a chance to crash.
//
// These crashes are so rare in debug mode that I've only seen them once. They
// happen a lot more often in release mode.

const workerModule = `
  await new Promise(resolve => setTimeout(resolve, 1000));
`;

// Iterating 10 times to increase the likelihood of triggering the crash, at
// least in release mode.
for (let i = 0; i < 10; i++) {
  const worker = new Worker(
    `data:application/javascript;base64,${btoa(workerModule)}`,
    { type: "module" },
  );
  worker.terminate();
}
