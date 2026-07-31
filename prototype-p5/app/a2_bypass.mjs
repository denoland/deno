// PROTOTYPE — wayfinder P5. Throwaway.
//
// A2: the bypass test. Same probes as A1, but now the package is DENIED:
//
//   PERMCAP=1 PERMCAP_DENY=npm:p5-denied ../../deno-permcap run -A \
//     --v8-flags=--expose-gc a2_bypass.mjs
//
// A row where `fetch` is "function" means a package holding no grant obtained
// authority by routing its read through that primitive. That is a bypass, and
// it falsifies D3.
//
// The mechanism under test is P1's Channel 1: a named property handler that
// resolves the reading package at accessor time. So a bypass here can only come
// from the stamp being wrong at that moment — either absent (read as
// first-party, unconfined) or belonging to someone else.

import { probeAll } from "p5-denied";

const rows = await probeAll();

console.log("\n=== A2: authority reachable while denied ===");
console.table(rows);

const bypasses = rows.filter((r) => r.fetch === "function");
const unattributed = rows.filter((r) => r.whoami === "<unattributed>");

console.log(`\nBYPASSES (denied package holding authority): ${bypasses.length}`);
for (const r of bypasses) console.log(`  !! ${r.ctx} -> fetch is ${r.fetch}, stamp ${r.whoami}`);

console.log(`\nunattributed-but-denied (latent, blocks on D7): ${unattributed.length}`);
for (const r of unattributed) console.log(`  ?  ${r.ctx}`);

if (bypasses.length === 0) {
  console.log("\nNo bypass found across the probed surface.");
}
