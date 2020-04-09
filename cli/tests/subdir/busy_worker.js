self.onmessage = function(evt) {
  console.error("worker entering busy loop");
  // infinite loop
  for (let i = 0; true; i++) {
    if (i % 1000 == 0) {
      postMessage(i);
    }
  }
}