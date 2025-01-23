// Copyright 2018-2025 the Deno authors. MIT license.

import * as punycode from "node:punycode";
import { assertEquals } from "@std/assert";

Deno.test("regression #19214", () => {
  const input = "个\uFFFD\uFFFD.hk";

  assertEquals(punycode.toASCII(input), "xn--ciq6844ba.hk");

  assertEquals(punycode.toUnicode("xn--ciq6844ba.hk"), input);
});

Deno.test("Decode empty input", () => {
  assertEquals(punycode.decode(""), "");
});
