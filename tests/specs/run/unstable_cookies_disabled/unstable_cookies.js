const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, typeof Deno.Cookie);
console.log(scope, typeof Deno.CookieJar);
console.log(scope, typeof Deno.CookieMap);

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
