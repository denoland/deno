// PROTOTYPE — wayfinder P5. Throwaway.
//
// A3: can a denied package obtain authority by re-entering a GRANTED package's
// async context? TC39 AsyncContext is not exposed in Deno, so the instrument is
// AsyncResource (captures getAsyncContext() at construction, restores it in
// runInAsyncScope) plus static AsyncLocalStorage.bind/snapshot.
//
//   PERMCAP=1 PERMCAP_DENY=npm:p5-denied ../../deno-permcap run -A a3_context_steal.mjs
//
// Row 0 must show fetch "function" (the granted package really is granted).
// Every other row must show "undefined". Anything else is a bypass.

import { stealAll } from "p5-denied/steal.mjs";

const rows = await stealAll();
console.log("\n=== A3: re-entering a granted package's async context ===");
console.table(rows);

const control = rows[0];
const rest = rows.slice(1);
const bypasses = rest.filter((r) => r.fetch === "function");

if (control.fetch !== "function") {
  console.log("\n!! CONTROL FAILED: the granted package does not hold authority; A3 proves nothing.");
} else {
  console.log(`\nBYPASSES: ${bypasses.length}`);
  for (const r of bypasses) console.log(`  !! ${r.ctx} -> fetch ${r.fetch}, stamp ${r.whoami}`);
  if (bypasses.length === 0) console.log("Context re-entry did not carry authority.");
}
