// PROTOTYPE — wayfinder P3. Throwaway.
// E6: what does each *installation shape* of Channel 1 cost, in a release
// build, booting from the startup snapshot?
//
// The variant is chosen by which binary you run (see run_p3.sh):
//   off       no named property handler anywhere
//   mask      masking interceptor on the global object template (P1's shape)
//   nonmask   NON_MASKING interceptor + side bag (ext/node/global.rs's shape)
//   accessor  per-property accessors on the live global (PERMCAP_ACCESSOR=1)
const N = 5_000_000;
const probe = Deno[Deno.internal].opPermcapProbe;

function bench(label, fn) {
  fn(100_000); // warm, and let the ICs settle
  let best = Infinity;
  let v;
  for (let r = 0; r < 3; r++) {
    const t0 = performance.now();
    v = fn(N);
    const t1 = performance.now();
    if (t1 - t0 < best) best = t1 - t0;
  }
  console.log(
    `${label.padEnd(34)} ${best.toFixed(1).padStart(8)} ms  ` +
      `${((best * 1e6) / N).toFixed(2).padStart(8)} ns/op  (${v})`,
  );
}

// --- what is actually installed ------------------------------------------
console.log(
  `# variant probe: typeof fetch=${typeof globalThis.fetch}` +
    ` whoami=${globalThis.__permcapWhoami}` +
    ` fetchIsOwn=${
      Object.prototype.hasOwnProperty.call(globalThis, "fetch")
    }` +
    ` fetchDesc=${
      Object.getOwnPropertyDescriptor(globalThis, "fetch")?.get
        ? "accessor"
        : Object.getOwnPropertyDescriptor(globalThis, "fetch")
        ? "data"
        : "absent"
    }`,
);

// --- global reads ---------------------------------------------------------
bench("guarded global read (fetch)", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) if (globalThis.fetch) c++;
  return c;
});
bench("unguarded global read (Math)", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) if (globalThis.Math) c++;
  return c;
});
bench("bare free-name read (Math)", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) if (Math) c++;
  return c;
});
bench("bare free-name read (fetch)", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) if (fetch) c++;
  return c;
});
bench("global miss (absent prop)", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) if (globalThis.__nope === undefined) c++;
  return c;
});
bench("local read baseline", (n) => {
  const m = Math;
  let c = 0;
  for (let i = 0; i < n; i++) if (m) c++;
  return c;
});

// --- D2's OpState current-package cell -------------------------------------
// This scales with authority *use*, not with global reads, and never touches
// V8's inline caches — a different cost shape from everything above.
bench("op, no bracket (today's check)", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) c += probe(false);
  return c;
});
bench("op + D2 cell set/restore", (n) => {
  let c = 0;
  for (let i = 0; i < n; i++) c += probe(true);
  return c;
});
