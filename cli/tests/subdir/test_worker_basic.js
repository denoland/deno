console.log("hello from test_worker_basic.js");

if (self.name !== "jsWorker") {
  throw Error(`Bad worker name: ${self.name}, expected jsWorker`);
}

onmessage = function (e) {
  postMessage(e.data);
  close();
};

onerror = function () {
  return false;
};
