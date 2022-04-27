self.onmessage = () => {
  console.log(9);
  console.error(10);
  self.postMessage({});
  self.close();
};
