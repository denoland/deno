const channel = new MessageChannel();

channel.port2.onmessage = (e) => {
  channel.port2.postMessage(e.data === "2");
  channel.port2.close();
};

self.postMessage("1", [channel.port1]);

self.onmessage = (e) => {
  const port1 = e.ports[0];
  port1.postMessage(e.data === "3");
  port1.close();
};
