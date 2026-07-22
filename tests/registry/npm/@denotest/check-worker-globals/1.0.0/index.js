if (typeof self !== "undefined") {
  throw new Error("self is defined");
}

if (typeof WorkerGlobalScope !== "undefined") {
  throw new Error("WorkerGlobalScope is defined");
}
