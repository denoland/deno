const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, Deno.AtomicOperation);
console.log(scope, Deno.Kv);
console.log(scope, Deno.KvListIterator);
console.log(scope, Deno.KvU64);
console.log(scope, Deno.openKv);

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
