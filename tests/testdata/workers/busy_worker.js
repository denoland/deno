self.onmessage = function (_evt) {
  // infinite loop
  for (let i = 0; true; i++) {
    if (i % 1000 == 0) {
      postMessage(i);
    }
  }
};
