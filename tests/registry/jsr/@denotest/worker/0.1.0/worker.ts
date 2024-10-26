self.onmessage = (evt) => {
  self.postMessage(evt.data.a + evt.data.b);
  self.close();
};
