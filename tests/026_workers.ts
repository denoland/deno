const jsWorker = new Worker("./tests/subdir/test_worker.js");
const tsWorker = new Worker("./tests/subdir/test_worker.ts");

setTimeout((): void => {
  jsWorker.onmessage = (e): void => {
    console.log("msg from js worker", e);
  };

  tsWorker.onmessage = (e): void => {
    console.log("msg from ts worker", e);
  };

  jsWorker.onerror = (e): void => {
    console.log("error from js worker", e);
  };

  tsWorker.onerror = (e): void => {
    console.log("error from ts worker", e);
  };

  jsWorker.postMessage("hello world");
  tsWorker.postMessage("hello world");
}, 1000);

setTimeout((): void => {
  jsWorker.postMessage("hello js after timeout");
}, 2250);

setTimeout((): void => {
  tsWorker.postMessage("hello ts after timeout");
}, 2500);
