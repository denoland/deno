// Copyright 2018-2026 the Deno authors. MIT license.
// FAIL

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}
function main() {
  assert(false);
}
main();
