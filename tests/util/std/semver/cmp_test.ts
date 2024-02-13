// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { Operator } from "./types.ts";
import { cmp } from "./cmp.ts";

Deno.test("cmp", async (t) => {
  const cases: [string, string, Operator, boolean][] = [
    ["1.0.0", "1.0.0", "", true],
    ["1.0.0", "1.0.1", "", false],
    ["1.0.0", "1.0.0", "=", true],
    ["1.0.0", "1.0.1", "=", false],
    ["1.0.0", "1.0.0", "==", true],
    ["1.0.0", "1.0.1", "==", false],
    ["1.0.0", "1.0.0", "===", true],
    ["1.0.0", "1.0.1", "===", false],
    ["1.0.0", "1.0.0", "!=", false],
    ["1.0.0", "1.0.1", "!=", true],
    ["1.0.0", "1.0.0", "!==", false],
    ["1.0.0", "1.0.1", "!==", true],
    ["1.0.0", "1.0.1", ">", false],
    ["1.0.1", "1.0.0", ">", true],
    ["1.0.0", "1.0.0", ">=", true],
    ["1.0.0", "1.0.1", ">=", false],
    ["1.0.0", "1.0.0", "<", false],
    ["1.0.0", "1.0.1", "<", true],
    ["1.0.0", "1.0.0", "<=", true],
    ["1.0.1", "1.0.0", "<=", false],
  ];
  for (const [v0, v1, op, expected] of cases) {
    await t.step(`${v0} ${op} ${v1} : ${expected}`, () => {
      const s0 = parse(v0);
      const s1 = parse(v1);
      const actual = cmp(s0, op, s1);
      assertEquals(actual, expected);
    });
  }
});

Deno.test("invalidCmpUsage", function () {
  assertThrows(
    () =>
      cmp(
        parse("1.2.3"),
        "a frog" as Operator,
        parse("4.5.6"),
      ),
    TypeError,
    "Invalid operator: a frog",
  );
});
