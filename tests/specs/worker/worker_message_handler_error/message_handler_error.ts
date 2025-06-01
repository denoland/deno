postMessage("ready");
onmessage = () => {
  throw new Error("bar");
};
