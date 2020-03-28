let thrown = false;

if (self.name !== "jsWorker") {
  throw Error(`Bad worker name: ${self.name}, expected jsWorker`);
}

onmessage = function (e) {
  if (thrown === false) {
    thrown = true;
    throw new SyntaxError("[test error]");
  }

  postMessage(e.data);
  close();
};

onerror = function () {
  return false;
};
