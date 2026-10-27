// Copyright 2018-2026 the Deno authors. MIT license.

const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, globalThis.OffscreenCanvasRenderingContext2D);
const canvas = new OffscreenCanvas(100, 100);
console.log(scope, canvas.getContext("2d"));

// `fonts` mirrors `document.fonts`, so it is only exposed on worker scopes.
// The main scope reaches the same object through `Deno.fonts`.
console.log(scope, globalThis.FontFace);
console.log(scope, globalThis.fonts);
console.log(scope, Deno.fonts);

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
