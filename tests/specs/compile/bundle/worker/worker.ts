self.onmessage = (e) => {
  (self as unknown as Worker).postMessage("pong-" + e.data);
};
