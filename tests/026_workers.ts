const jsWorker = new Worker("./tests/subdir/test_worker.js");
const tsWorker = new Worker("./tests/subdir/test_worker.ts");

let tsReceived = 0;

tsWorker.onmessage = (e): void => {
  console.log("Received ts: " + e.data);
  tsReceived++;

  if (tsReceived === 1) {
    tsWorker.postMessage("trigger error");
  } else if (tsReceived === 3) {
    tsWorker.postMessage("exit");
  } else {
    tsWorker.postMessage("Hello again!");
  }
};

let jsReceived = false;

jsWorker.onmessage = (e): void => {
  console.log("Received js: " + e.data);
  if (!jsReceived) {
    jsReceived = true;
    jsWorker.postMessage("trigger error");
  }
};

// jsWorker.postMessage("Hello World");
tsWorker.postMessage("Hello World");
