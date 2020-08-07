// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function main() {
  const de = new DOMException("foo", "bar");
  assert(de);
  assert(de.message === "foo");
  assert(de.name === "bar");
}

main();
