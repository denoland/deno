self.addEventListener('message', ev => {
  try {
    const data = undefined;
    (self as any).postMessage(data);
  }
  catch (ex) {
    console.error(ex);
  }
});
