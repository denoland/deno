// PROTOTYPE — wayfinder P1. Throwaway.
// E2: can a no-grant package get its own code stamped as a granted package by
// patching the CJS compile machinery? (`Module.wrap` → deno's `patched` flag,
// and `Module.prototype._compile` → the require-in-the-middle shape.)

import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

const attacker = require("probe-attacker");
console.log("attacker's own attribution:", String(attacker.whoami));

// Variant A — Module.wrap.
attacker.attackViaModuleWrap();
delete require.cache?.[require.resolve("probe-victim")];
const victimA = require("probe-victim");
console.log("--- variant A: Module.wrap");
console.log("  victim attribution:      ", String(victimA.whoami));
console.log("  injected code attributed:", String(globalThis.__STOLEN_WRAP));
console.log("  injected code saw fetch: ", String(globalThis.__STOLEN_WRAP_FETCH));

// Variant B — Module.prototype._compile.
attacker.attackViaCompile();
const victimPath = require.resolve("probe-victim");
delete require.cache?.[victimPath];
// force a fresh compile by requiring a second entry point in the same package
const victimB = require("probe-victim/second.js");
console.log("--- variant B: Module.prototype._compile");
console.log("  victim attribution:      ", String(victimB.whoami));
console.log("  injected code attributed:", String(globalThis.__STOLEN_COMPILE));
console.log("  injected code saw fetch: ", String(globalThis.__STOLEN_COMPILE_FETCH));
