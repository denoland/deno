// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const ops = Deno.core.ops;
const queueMicrotask = globalThis.queueMicrotask || process.nextTick;
let [total, count] = typeof Deno !== "undefined"
  ? Deno.args
  : [process.argv[2], process.argv[3]];

total = total ? parseInt(total, 0) : 50;
count = count ? parseInt(count, 10) : 1000000;

const sizeOutBuffer = new Uint32Array(1);
const forgivingBase64Decode = "op_base64_decode_start" in Deno.core.ops
  ? function forgivingBase64Decode(data) {
    const rid = ops.op_base64_decode_start(data, sizeOutBuffer);
    const resultBuffer = new Uint8Array(sizeOutBuffer[0]);
    ops.op_base64_decode_finish(rid, resultBuffer);
    return resultBuffer;
  }
  : function forgivingBase64DecodeMain(data) {
    return ops.op_base64_decode(data);
  };

function bench(fun) {
  const start = Date.now();
  for (let i = 0; i < count; i++) fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  if (--total) queueMicrotask(() => bench(fun));
}

const data = btoa("hello world");

bench(() => forgivingBase64Decode(data));
