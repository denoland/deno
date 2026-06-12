const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, typeof Deno.Cookie);
console.log(scope, typeof Deno.CookieJar);
console.log(scope, typeof Deno.CookieMap);

const jar = new Deno.CookieJar();
jar.setCookie("a=1; Path=/", "https://example.com/");
console.log(scope, jar.getCookieString("https://example.com/"));
jar.close();

const cookies = new Deno.CookieMap("a=1; b=2");
console.log(scope, cookies.get("b"));

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
