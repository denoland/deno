onmessage = function (e) {
  postMessage(!!self.crypto);
};
