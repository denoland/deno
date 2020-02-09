// Tests basic postMessage, close, onmessage
const jsWorker = new Worker("./subdir/test_worker_basic.js", {
  type: "module",
  name: "jsWorker"
});

jsWorker.onmessage = (e): void => {
  console.log("main recv: " + e.data);
};

jsWorker.postMessage("msg1");
