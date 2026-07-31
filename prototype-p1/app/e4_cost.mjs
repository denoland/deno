// PROTOTYPE — wayfinder P1. Throwaway.
// E4: what does a named property handler on the global object cost?
// Same binary both ways: PERMCAP unset installs no interceptor at all.
const N = 5_000_000;

function bench(label, fn) {
  fn(1000); // warm
  const t0 = performance.now();
  const v = fn(N);
  const t1 = performance.now();
  console.log(
    `${label.padEnd(34)} ${((t1 - t0)).toFixed(1).padStart(8)} ms  ` +
      `${(((t1 - t0) * 1e6) / N).toFixed(2)} ns/op  (${v})`,
  );
}

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
