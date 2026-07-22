// Copyright 2018-2026 the Deno authors. MIT license.
// https://github.com/denoland/deno_core/issues/742
console.log("1");
Object.defineProperty(Promise, Symbol.species, { value: 0 });
console.log("2");
