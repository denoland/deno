const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, Deno.HttpClient);
console.log(scope, Deno.createHttpClient);
console.log(scope, Deno.http?.HttpConn);
console.log(scope, Deno.http?._ws);
console.log(scope, Deno.http?.serve);
console.log(scope, Deno.http?.upgradeHttp);
console.log(scope, Deno.http?.upgradeWebSocket);
console.log(scope, Deno.upgradeHttp);

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
