new BroadcastChannel("intercom").onmessage = function (e) {
  this.postMessage(e.data);
};

self.postMessage("go");
