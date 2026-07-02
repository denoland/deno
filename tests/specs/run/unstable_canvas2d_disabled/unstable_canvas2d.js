// Copyright 2018-2026 the Deno authors. MIT license.

const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

console.log(scope, globalThis.OffscreenCanvasRenderingContext2D);
const canvas = new OffscreenCanvas(100, 100);
console.log(scope, canvas.getContext("2d"));

if (scope === "worker") {
  postMessage("done");
} else {
  const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
  worker.onmessage = () => Deno.exit(0);
}
