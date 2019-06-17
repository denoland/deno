const jsWorker = new Worker("./tests/subdir/test_worker.js");
const tsWorker = new Worker("./tests/subdir/test_worker.ts");

jsWorker.onmessage = (e: MessageEvent): void => {
  console.log("msg from js worker", e);
};

tsWorker.onmessage = (e: MessageEvent): void => {
  console.log("msg from ts worker", e);
};

jsWorker.onerror = (e: ErrorEvent): void => {
  console.log("error from js worker", e);
};

tsWorker.onerror = (e: ErrorEvent): void => {
  console.log("error from ts worker", e);
};

jsWorker.postMessage("hello world");
tsWorker.postMessage("hello world");

setTimeout((): void => {
  jsWorker.postMessage("hello js after timeout");
}, 250);

setTimeout((): void => {
  tsWorker.postMessage("hello ts after timeout");
}, 500);
