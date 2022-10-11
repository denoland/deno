// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const queueMicrotask = globalThis.queueMicrotask || process.nextTick;
let [total, count] = typeof Deno !== "undefined"
  ? Deno.args
  : [process.argv[2], process.argv[3]];

total = total ? parseInt(total, 0) : 50;
count = count ? parseInt(count, 10) : 1000000;

const promises = new Array(count);
async function bench(fun) {
  const start = Date.now();
  for (let i = 0; i < count; i++) promises[i] = fun();
  await Promise.all(promises);
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  if (--total) await bench(fun);
}

const core = Deno.core;
const ops = core.ops;
if (core.opFastAsync) {
  const opCall = (a, b) => ops.op_void_async_deferred(a, b);
  bench(() => core.opFastAsync(opCall));
} else {
  bench(() => core.opAsync("op_void_async_deferred"));
}
