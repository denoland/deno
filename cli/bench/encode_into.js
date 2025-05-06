// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console no-process-global

let [total, count] = typeof Deno !== "undefined"
  ? Deno.args
  : [process.argv[2], process.argv[3]];

total = total ? parseInt(total, 0) : 50;
count = count ? parseInt(count, 10) : 10000000;

function bench(fun) {
  const start = Date.now();
  for (let i = 0; i < count; i++) fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  if (--total) bench(fun);
}

const encoder = new TextEncoder();
const data = "hello world";
const out = new Uint8Array(100);

bench(() => encoder.encodeInto(data, out));
