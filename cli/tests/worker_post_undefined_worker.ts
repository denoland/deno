self.addEventListener("message", (ev) => {
  try {
    const data = undefined;
    self.postMessage(data);
  } catch (ex) {
    console.error(ex);
  }
});
