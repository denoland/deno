// Regression test for https://github.com/denoland/deno/issues/34187
// `process._getActiveHandles()` must be callable from the `'exit'` event
// handler in a CJS module that lazy-loads `node:http`.

setImmediate(function () {
  require("http");
});

var i = setInterval(function () {
  setImmediate(process.exit);
}, 10);
i.unref();

process.on("exit", logstuff);

function logstuff() {
  if (typeof process._getActiveHandles !== "function") {
    throw new TypeError("process._getActiveHandles is not a function");
  }
  if (typeof process._getActiveRequests !== "function") {
    throw new TypeError("process._getActiveRequests is not a function");
  }
  if (typeof process.getActiveResourcesInfo !== "function") {
    throw new TypeError("process.getActiveResourcesInfo is not a function");
  }
  console.log("handles=" + process._getActiveHandles().length);
  console.log("requests=" + process._getActiveRequests().length);
  console.log("ok");
}
