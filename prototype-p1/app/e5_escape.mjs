// PROTOTYPE - wayfinder P1. probe-esm is on the deny list for this run.
import { escapeViaWorker, ownView } from "./node_modules/probe-esm/escape.mjs";
console.log("package's own view: ", JSON.stringify(ownView()));
console.log("via spawned worker: ", JSON.stringify(await escapeViaWorker()));
console.log("app (first-party):  ", JSON.stringify({ whoami: String(globalThis.__permcapWhoami), fetch: typeof globalThis.fetch }));
