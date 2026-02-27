// Copyright 2018-2026 the Deno authors. MIT license.
export const foo = "foo";
export const bar = 123;
export function blah(a) {
  Deno.core.print(a);
}
export default { foo, bar, blah };
