self.onmessage = () => {
  console.log(8);
  console.error(9);
  self.postMessage({});
  self.close();
};
