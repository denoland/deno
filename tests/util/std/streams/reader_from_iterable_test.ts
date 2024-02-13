// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { readerFromIterable } from "./reader_from_iterable.ts";

Deno.test("[streams] readerFromIterable()", async function () {
  const reader = readerFromIterable((function* () {
    const encoder = new TextEncoder();
    for (const string of ["hello", "deno", "foo"]) {
      yield encoder.encode(string);
    }
  })());

  const readStrings = [];
  const decoder = new TextDecoder();
  const p = new Uint8Array(4);
  while (true) {
    const n = await reader.read(p);
    if (n === null) {
      break;
    }
    readStrings.push(decoder.decode(p.slice(0, n)));
  }
  assertEquals(readStrings, ["hell", "o", "deno", "foo"]);
});
