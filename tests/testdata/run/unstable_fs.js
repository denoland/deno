const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, Deno.flock);
console.log(scope, Deno.flockSync);
console.log(scope, Deno.funlock);
console.log(scope, Deno.funlockSync);
console.log(scope, Deno.umask);

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
