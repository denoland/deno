// Reading `__proto__` returns `undefined` while the accessor is disabled (the
// default), so `who.__proto__.constructor` throws right at the access site.
// Because the crashing line itself mentions `__proto__`, the uncaught-error
// formatter suggests `--unstable-unsafe-proto`. This mirrors the real-world
// crash in pnpm 11 (denoland/deno#34694).
const who = {};
who.__proto__.constructor;
