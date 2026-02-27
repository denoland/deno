// Copyright 2018-2025 the Deno authors. MIT license.
setTimeout(() => console.log("a"), 1000);
setTimeout(() => console.log("b"), 2000);
// Make these long enough that we'll never hit them
const c = setTimeout(() => console.log("c"), 120_000);
const d = setTimeout(() => console.log("d"), 150_000);
// c should not run, if d is cancelled
Deno.unrefTimer(c);
clearTimeout(d);
