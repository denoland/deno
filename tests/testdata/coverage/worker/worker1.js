self.onmessage = () => {
  postMessage("hello from worker");
};

setTimeout(() => {
  self.close();
}, 100);
