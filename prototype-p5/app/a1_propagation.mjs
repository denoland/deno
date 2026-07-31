// PROTOTYPE — wayfinder P5. Throwaway.
//
// A1: the stamp-in-force table. Run WITHOUT PERMCAP_DENY, so nothing is denied
// and the only question is attribution: for each scheduling / context-
// propagation primitive, what does GetCurrentHostDefinedOptions() report?
//
//   PERMCAP=1 ../../deno-permcap run -A --v8-flags=--expose-gc a1_propagation.mjs
//
// Read the `whoami` column. Anything that is not "npm:p5-denied" is a finding:
//
//   npm:p5-denied     — sound. The stamp followed the running script.
//   <unattributed>    — DANGEROUS. D1 reads an absent stamp as first-party,
//                       i.e. unconfined. Sound only if D7 lands positive
//                       first-party stamping first.
//   <never fired>     — the primitive is unavailable or the callback never ran;
//                       not evidence either way, note and move on.

import { probeAll } from "p5-denied";

const rows = await probeAll();

console.log("\n=== A1: stamp in force, nothing denied ===");
console.log("app (first-party) reads as:", String(globalThis.__permcapWhoami));
console.table(rows.map(({ ctx, whoami }) => ({ ctx, whoami })));

const unattributed = rows.filter((r) => r.whoami === "<unattributed>");
const other = rows.filter(
  (r) =>
    r.whoami !== "npm:p5-denied" &&
    r.whoami !== "<unattributed>" &&
    r.whoami !== "<never fired>" &&
    !r.whoami.startsWith("threw:")
);

console.log(`\nunattributed contexts: ${unattributed.length}`);
for (const r of unattributed) console.log("  ! " + r.ctx);
console.log(`mis-attributed contexts: ${other.length}`);
for (const r of other) console.log(`  ! ${r.ctx} -> ${r.whoami}`);
