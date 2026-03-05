// Copyright 2018-2026 the Deno authors. MIT license.
console.log("deferred module evaluated");
export const value = 42;
export function add(a, b) {
  return a + b;
}
