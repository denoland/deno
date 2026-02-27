// Copyright 2018-2025 the Deno authors. MIT license.
setTimeout(() => console.log("a"), 1000);
setTimeout(() => console.log("b"), 2000);
// Make this long enough that we'll never hit it
const c = setTimeout(() => console.log("c"), 120_000);
Deno.core.unrefTimer(c);
