// Copyright 2018-2026 the Deno authors. MIT license.
console.log("lib.js before");

export function sleep(timeout) {
  return new Promise((resolve) => {
    Deno.core.createTimer(resolve, timeout, undefined, false, true);
  });
}
await sleep(100);

console.log("lib.js after");

const abc = 1 + 2;
export function add(a, b) {
  console.log(`abc: ${abc}`);
  return a + b;
}
