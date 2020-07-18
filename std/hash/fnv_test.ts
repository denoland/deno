// Ported from Go:
// https://github.com/golang/go/tree/go1.13.10/src/hash/fnv/fnv_test.go
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { Fnv32, Fnv32a, Fnv64, Fnv64a } from "./fnv.ts";

const golden32 = [
  ["", [0x81, 0x1c, 0x9d, 0xc5]],
  ["a", [0x05, 0x0c, 0x5d, 0x7e]],
  ["ab", [0x70, 0x77, 0x2d, 0x38]],
  ["abc", [0x43, 0x9c, 0x2f, 0x4b]],
  ["deno", [0x6e, 0xd5, 0xa7, 0xa9]],
];

const golden32a = [
  ["", [0x81, 0x1c, 0x9d, 0xc5]],
  ["a", [0xe4, 0x0c, 0x29, 0x2c]],
  ["ab", [0x4d, 0x25, 0x05, 0xca]],
  ["abc", [0x1a, 0x47, 0xe9, 0x0b]],
  ["deno", [0x8e, 0xf6, 0x47, 0x11]],
];

const golden64 = [
  ["", [0xcb, 0xf2, 0x9c, 0xe4, 0x84, 0x22, 0x23, 0x25]],
  ["a", [0xaf, 0x63, 0xbd, 0x4c, 0x86, 0x01, 0xb7, 0xbe]],
  ["ab", [0x08, 0x32, 0x67, 0x07, 0xb4, 0xeb, 0x37, 0xb8]],
  ["abc", [0xd8, 0xdc, 0xca, 0x18, 0x6b, 0xaf, 0xad, 0xcb]],
  ["deno", [0x14, 0xed, 0xb2, 0x7e, 0xec, 0xda, 0xad, 0xc9]],
];

const golden64a = [
  ["", [0xcb, 0xf2, 0x9c, 0xe4, 0x84, 0x22, 0x23, 0x25]],
  ["a", [0xaf, 0x63, 0xdc, 0x4c, 0x86, 0x01, 0xec, 0x8c]],
  ["ab", [0x08, 0x9c, 0x44, 0x07, 0xb5, 0x45, 0x98, 0x6a]],
  ["abc", [0xe7, 0x1f, 0xa2, 0x19, 0x05, 0x41, 0x57, 0x4b]],
  ["deno", [0xa5, 0xd9, 0xfb, 0x67, 0x42, 0x6e, 0x48, 0xb1]],
];

Deno.test("[hash/fnv] testFnv32", () => {
  for (const [input, output] of golden32) {
    const fnv = new Fnv32();
    fnv.write(new TextEncoder().encode(input as string));
    assertEquals(fnv.sum(), output);
  }
});

Deno.test("[hash/fnv] testFnv32a", () => {
  for (const [input, output] of golden32a) {
    const fnv = new Fnv32a();
    fnv.write(new TextEncoder().encode(input as string));
    assertEquals(fnv.sum(), output);
  }
});

Deno.test("[hash/fnv] testFnv64", () => {
  for (const [input, output] of golden64) {
    const fnv = new Fnv64();
    fnv.write(new TextEncoder().encode(input as string));
    assertEquals(fnv.sum(), output);
  }
});

Deno.test("[hash/fnv] testFnv64a", () => {
  for (const [input, output] of golden64a) {
    const fnv = new Fnv64a();
    fnv.write(new TextEncoder().encode(input as string));
    assertEquals(fnv.sum(), output);
  }
});

Deno.test("[hash/fnv] testFnv32WriteChain", () => {
  const fnv = new Fnv32();
  fnv
    .write(new TextEncoder().encode("d"))
    .write(new TextEncoder().encode("e"))
    .write(new TextEncoder().encode("n"))
    .write(new TextEncoder().encode("o"));
  assertEquals(fnv.sum(), [0x6e, 0xd5, 0xa7, 0xa9]);
});

Deno.test("[hash/fnv] testFnv32aWriteChain", () => {
  const fnv = new Fnv32a();
  fnv
    .write(new TextEncoder().encode("d"))
    .write(new TextEncoder().encode("e"))
    .write(new TextEncoder().encode("n"))
    .write(new TextEncoder().encode("o"));
  assertEquals(fnv.sum(), [0x8e, 0xf6, 0x47, 0x11]);
});

Deno.test("[hash/fnv] testFnv64WriteChain", () => {
  const fnv = new Fnv64();
  fnv
    .write(new TextEncoder().encode("d"))
    .write(new TextEncoder().encode("e"))
    .write(new TextEncoder().encode("n"))
    .write(new TextEncoder().encode("o"));
  assertEquals(fnv.sum(), [0x14, 0xed, 0xb2, 0x7e, 0xec, 0xda, 0xad, 0xc9]);
});

Deno.test("[hash/fnv] testFnv64aWriteChain", () => {
  const fnv = new Fnv64a();
  fnv
    .write(new TextEncoder().encode("d"))
    .write(new TextEncoder().encode("e"))
    .write(new TextEncoder().encode("n"))
    .write(new TextEncoder().encode("o"));
  assertEquals(fnv.sum(), [0xa5, 0xd9, 0xfb, 0x67, 0x42, 0x6e, 0x48, 0xb1]);
});
