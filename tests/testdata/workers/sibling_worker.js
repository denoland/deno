onmessage = (e) => {
  postMessage(e.data);
  close();
};
