// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const count = 100000;

const start = Date.now();
for (let i = 0; i < count; i++) console.log("Hello World");
const elapsed = Date.now() - start;
const rate = Math.floor(count / (elapsed / 1000));
console.log(`time ${elapsed} ms rate ${rate}`);
