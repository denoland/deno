let thrown = false;

// TODO(bartlomieju): fix panic if this error is actually thrown
if (self.name !== "jsWorker") {
  throw Error(`Bad worker name: ${self.name}, expected jsWorker`);
}

onmessage = function(e) {
  console.log(e.data);

  if (thrown === false) {
    thrown = true;
    throw new SyntaxError("[test error]");
  }

  postMessage(e.data);

  close();
};

onerror = function() {
  console.log("called onerror in worker");
  return false;
};
