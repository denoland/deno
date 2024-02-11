if (self.name !== "") {
  throw Error(`Bad worker name: ${self.name}, expected empty string.`);
}

onmessage = function (e) {
  postMessage(e.data);
  close();
};
