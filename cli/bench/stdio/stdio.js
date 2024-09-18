// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
//
// From https://github.com/just-js/benchmarks/tree/main/01-stdio

// deno-lint-ignore-file no-console

const blocksize = parseInt(Deno.args[0] || 65536);
const buf = new Uint8Array(blocksize);
let size = 0;
let reads = 0;

let n = Deno.stdin.readSync(buf);
while (n > 0) {
  reads++;
  size += n;
  n = Deno.stdin.readSync(buf);
}
if (n < 0) throw new Error("Bad Read");
console.log(`size ${size} reads ${reads} blocksize ${blocksize}`);
