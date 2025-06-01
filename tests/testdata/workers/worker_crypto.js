self.crypto.getRandomValues(new Uint8Array(16));

onmessage = function () {
  postMessage(!!self.crypto);
};
