let thrown = false;

if (self.name !== "jsWorker") {
  throw Error(`Bad worker name: ${self.name}, expected jsWorker`);
}

onmessage = function(e) {
  console.log("calling onmessage js!");

  if (thrown === false) {
    thrown = true;
    throw new SyntaxError("[test error]");
  }

  postMessage(e.data);
  console.log("calling close in js!");
  close();
};

onerror = function() {
  return false;
};
