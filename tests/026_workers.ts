const jsWorker = new Worker("./tests/subdir/test_worker.js");
const tsWorker = new Worker("./tests/subdir/test_worker.ts");

jsWorker.onmessage = e => {
  console.log("msg from js worker", e);
};

tsWorker.onmessage = e => {
  console.log("msg from ts worker", e);
};

jsWorker.onerror = e => {
  console.log("error from js worker", e);
};

tsWorker.onerror = e => {
  console.log("error from ts worker", e);
};

jsWorker.postMessage("hello world");
tsWorker.postMessage("hello world");

setTimeout(() => {
  jsWorker.postMessage("hello js after timeout");
}, 250);

setTimeout(() => {
  tsWorker.postMessage("hello ts after timeout");
}, 500);
