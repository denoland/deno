const jsWorker = new Worker("./subdir/test_worker.js");
const tsWorker = new Worker("./subdir/test_worker.ts");

tsWorker.onmessage = (e): void => {
  console.log("Received ts: " + e.data);
};

jsWorker.onmessage = (e): void => {
  console.log("Received js: " + e.data);

  tsWorker.postMessage("Hello World");
};

jsWorker.onerror = (): void => {
  console.error("on error called!");
  tsWorker.postMessage("Hello World");
}

jsWorker.postMessage("Hello World");
