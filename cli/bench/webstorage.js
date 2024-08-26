// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

// Note: when benchmarking across different Deno version, make sure to clear
// the DENO_DIR cache.
let [total, count] = typeof Deno !== "undefined" ? Deno.args : [];

total = total ? parseInt(total, 0) : 50;
count = count ? parseInt(count, 10) : 1000000;

function bench(fun) {
  const start = Date.now();
  for (let i = 0; i < count; i++) fun(i);
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  if (--total) queueMicrotask(() => bench(fun));
}

localStorage.clear();
localStorage.setItem("foo", "bar");
bench(() => localStorage.getItem("foo"));
