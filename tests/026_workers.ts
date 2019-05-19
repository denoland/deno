const jsWorker = new Worker("./tests/subdir/test_worker.js");
const tsWorker = new Worker("./tests/subdir/test_worker.ts");

tsWorker.onmessage = (e): void => {
  console.log("Received ts: " + e.data);
};

jsWorker.onmessage = (e): void => {
  console.log("Received js: " + e.data);

  tsWorker.postMessage("Hello World");
};

jsWorker.postMessage("Hello World");
