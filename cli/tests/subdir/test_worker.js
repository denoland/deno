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

onerror = function(e, a, d, f, h) {
  console.log("caught in worker onerror");
  return false;
};
