// PROTOTYPE — wayfinder P1. Throwaway.
// E1: does GetCurrentHostDefinedOptions attribute the *reading* package
// reliably, across every context the mechanism has to survive?

import * as esm from "probe-esm";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const rows = [];
const row = (ctx, got) => rows.push({ ctx, attributed: String(got) });

row("app (first-party) top level", globalThis.__permcapWhoami);
row("esm pkg, module top level", esm.whoamiTopLevel);
row("esm pkg, sync call from app", esm.whoamiSync());
row("esm pkg, after await", await esm.whoamiAfterAwait());
row("esm pkg, setTimeout callback", await esm.whoamiAfterTimer());
row("esm pkg, queueMicrotask", await esm.whoamiInMicrotask());
row("esm pkg, Promise.then", await esm.whoamiAfterThen());
row("esm pkg, after real op I/O", await esm.whoamiAfterIO());
row("esm pkg, direct eval", esm.whoamiViaDirectEval());
row("esm pkg, indirect eval", esm.whoamiViaIndirectEval());
row("esm pkg, new Function", esm.whoamiViaFunction());
row("esm pkg, dynamic import(data:)", await esm.whoamiViaDynamicImport());

// A getter defined in the package but invoked by ext: JS on the app's behalf.
const withGetter = esm.objectWithGetter();
row("getter read by app code", withGetter.probe);
row("getter read via JSON.stringify (v8 builtin)", JSON.parse(JSON.stringify({ p: withGetter.probe })).p);
row("toString invoked by ext: (new URL)", (() => {
  try {
    return new URL("http://example.com/" + withGetter);
  } catch (e) {
    return "threw: " + e.message;
  }
})().pathname.slice(1));
row("getter invoked by ext: (Headers init)", (() => {
  const h = new Headers({ "x-probe": String(withGetter) });
  return h.get("x-probe");
})());
row("getter invoked by ext: (structuredClone)", (() => {
  try {
    return structuredClone({ get p() { return globalThis.__permcapWhoami; } }).p;
  } catch (e) {
    return "threw: " + e.message;
  }
})());

// CJS half.
const cjs = require("probe-cjs");
row("cjs pkg, module top level", cjs.whoamiTopLevel);
row("cjs pkg, sync call from app", cjs.whoamiSync());
row("cjs pkg, after await", await cjs.whoamiAfterAwait());
row("cjs pkg, setTimeout callback", await cjs.whoamiAfterTimer());
row("cjs pkg, required child module", cjs.whoamiViaRequiredChild());

// Callback invoked from deep inside a real npm package's stack (lodash), so the
// running script at read time is lodash, not us.
const _ = require("lodash");
row("app callback run by lodash", _.map([1], () => globalThis.__permcapWhoami)[0]);

// Worker.
const workerUrl = new URL("./worker_probe.mjs", import.meta.url);
const worker = new Worker(workerUrl, { type: "module" });
row("worker top level", await new Promise((resolve) => {
  worker.onmessage = (e) => { worker.terminate(); resolve(e.data); };
}));

console.table(rows);
