// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const queueMicrotask = globalThis.queueMicrotask || process.nextTick;
let [total, count] = typeof Deno !== "undefined"
  ? Deno.args
  : [process.argv[2], process.argv[3]];

total = total ? parseInt(total, 0) : 50;
count = count ? parseInt(count, 10) : 10000;

async function bench(fun) {
  const start = Date.now();
  for (let i = 0; i < count; i++) await fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  if (--total) queueMicrotask(() => bench(fun));
}

const file = await Deno.open("/tmp/1.txt", { write: true, create: true });
bench(() => file.write(new Uint8Array(1024)));

// bench(() => Deno.writeFile("/dev/null", new Uint8Array(10)));

// (async () => {
//   const fs = require("fs").promises;
//   const fd = await fs.open("/tmp/1.txt", "w");
//   bench(() => fd.write(new Uint8Array(1024)));
// })();

// bench(() => fs.writeFile("/dev/null", new Uint8Array(10)));
