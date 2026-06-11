const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, typeof globalThis.HTMLRewriter);

if (scope === "worker") {
  const output = new HTMLRewriter()
    .on("a", {
      element(element) {
        element.setAttribute("href", "https://deno.com");
      },
    })
    .transform('<a href="http://example.com">a link</a>');
  console.log(output);
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
