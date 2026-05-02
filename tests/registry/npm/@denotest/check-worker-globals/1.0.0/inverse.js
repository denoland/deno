if (typeof self === "undefined") {
  throw new Error("self is not defined");
}

if (typeof WorkerGlobalScope === "undefined") {
  throw new Error("WorkerGlobalScope is not defined");
}
