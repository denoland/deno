let thrown = false;

onmessage = function(e) {
  console.log(e.data);

  if (thrown === false) {
    thrown = true;
    throw new SyntaxError("[test error]");
  }

  postMessage(e.data);

  workerClose();
};

onerror = function() {
  console.log("called onerror in worker");
  return false;
};
