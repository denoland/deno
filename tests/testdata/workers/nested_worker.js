// Specifier should be resolved relative to current file
const jsWorker = new Worker(
  import.meta.resolve("./sibling_worker.js"),
  { type: "module", name: "sibling" },
);

jsWorker.onerror = (_e) => {
  postMessage({ type: "error" });
};

jsWorker.onmessage = (e) => {
  postMessage({ type: "msg", text: e.data });
  close();
};

onmessage = function (e) {
  jsWorker.postMessage(e.data);
};
