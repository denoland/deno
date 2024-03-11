import * as punycode from "node:punycode";
import { assertEquals } from "@std/assert/mod.ts";

Deno.test("regression #19214", () => {
  const input = "个��.hk";

  assertEquals(punycode.toASCII(input), "xn--ciq6844ba.hk");

  assertEquals(punycode.toUnicode("xn--ciq6844ba.hk"), input);
});

Deno.test("Decode empty input", () => {
  assertEquals(punycode.decode(""), "");
});
