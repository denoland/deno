// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// Run with: deno run -A ./cli/bench/deno_common.js
function benchSync(name, n, innerLoop) {
  const t1 = Date.now();
  for (let i = 0; i < n; i++) {
    innerLoop(i);
  }
  const t2 = Date.now();
  console.log(benchStats(name, n, t1, t2));
}

async function benchAsync(name, n, innerLoop) {
  const t1 = Date.now();
  for (let i = 0; i < n; i++) {
    await innerLoop(i);
  }
  const t2 = Date.now();
  console.log(benchStats(name, n, t1, t2));
}

// Parallel version benchAsync
async function benchAsyncP(name, n, p, innerLoop) {
  const range = new Array(p).fill(null);
  const t1 = Date.now();
  for (let i = 0; i < n / p; i++) {
    await Promise.all(range.map(() => innerLoop()));
  }
  const t2 = Date.now();
  console.log(benchStats(name, n, t1, t2));
}

function benchStats(name, n, t1, t2) {
  const dt = (t2 - t1) / 1e3;
  const r = n / dt;
  const ns = Math.floor(dt / n * 1e9);
  return `${name}:${" ".repeat(20 - name.length)}\t` +
    `n = ${n}, dt = ${dt.toFixed(3)}s, r = ${r.toFixed(0)}/s, t = ${ns}ns/op`;
}

function benchUrlParse() {
  benchSync("url_parse", 5e4, (i) => {
    new URL(`http://www.google.com/${i}`);
  });
}

function benchDateNow() {
  benchSync("date_now", 5e5, () => {
    Date.now();
  });
}

function benchPerfNow() {
  benchSync("perf_now", 5e5, () => {
    performance.now();
  });
}

function benchWriteNull() {
  // Not too large since we want to measure op-overhead not sys IO
  const dataChunk = new Uint8Array(100);
  const file = Deno.openSync("/dev/null", { write: true });
  benchSync("write_null", 5e5, () => {
    Deno.writeSync(file.rid, dataChunk);
  });
  Deno.close(file.rid);
}

function benchReadZero() {
  const buf = new Uint8Array(100);
  const file = Deno.openSync("/dev/zero");
  benchSync("read_zero", 5e5, () => {
    Deno.readSync(file.rid, buf);
  });
  Deno.close(file.rid);
}

function benchRead128k() {
  return benchAsync(
    "read_128k",
    5e4,
    () => Deno.readFile("./cli/bench/testdata/128k.bin"),
  );
}

function benchRequestNew() {
  return benchSync("request_new", 5e5, () => new Request("https://deno.land"));
}

function benchOpVoidSync() {
  return benchSync("op_void_sync", 1e7, () => Deno.core.opSync("op_void_sync"));
}

function benchOpVoidAsync() {
  return benchAsyncP(
    "op_void_async",
    1e6,
    1e3,
    () => Deno.core.opAsync("op_void_async"),
  );
}

async function main() {
  // v8 builtin that's close to the upper bound non-NOPs
  benchDateNow();
  // Void ops measure op-overhead
  benchOpVoidSync();
  await benchOpVoidAsync();
  // A very lightweight op, that should be highly optimizable
  benchPerfNow();
  // A common "language feature", that should be fast
  // also a decent representation of a non-trivial JSON-op
  benchUrlParse();
  // IO ops
  benchReadZero();
  benchWriteNull();
  await benchRead128k();
  benchRequestNew();
}
await main();
