const code = `
self.onmessage = function(e) {
  postMessage({ result: e.data });
};
`;
const blob = new Blob([code], { type: "text/javascript" });
const url = URL.createObjectURL(blob);

const worker = new Worker(url);
const { promise, resolve } = Promise.withResolvers();
worker.onmessage = (event) => {
  console.log(JSON.stringify(event.data));
  worker.terminate();
  resolve();
};
worker.postMessage("hello");
await promise;
