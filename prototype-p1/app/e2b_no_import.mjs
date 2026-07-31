// PROTOTYPE - wayfinder P1.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const a = require("probe-attacker/no_import.js");
console.log("reached Module via module.constructor:", a.attack());
const v = require("probe-victim/third.js");
console.log("victim attribution:      ", String(v.whoami));
console.log("injected code attributed:", String(globalThis.__STOLEN_NOIMPORT));
console.log("injected code saw fetch: ", String(globalThis.__STOLEN_NOIMPORT_FETCH));
