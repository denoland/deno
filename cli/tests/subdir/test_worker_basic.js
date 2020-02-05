console.log("hello from test_worker_basic.js");

// TODO(bartlomieju): add test for throwing in web worker
if (self.name !== "jsWorker") {
  throw Error(`Bad worker name: ${self.name}, expected jsWorker`);
}

onmessage = function(e) {
  console.log("jsWorker onmessage", e.data);
  postMessage(e.data);
  close();
};

onerror = function() {
  console.log("called onerror in worker");
  return false;
};
