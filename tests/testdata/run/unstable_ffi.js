const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, Deno.UnsafeCallback);
console.log(scope, Deno.UnsafeFnPointer);
console.log(scope, Deno.UnsafePointer);
console.log(scope, Deno.UnsafePointerView);
console.log(scope, Deno.dlopen);

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
