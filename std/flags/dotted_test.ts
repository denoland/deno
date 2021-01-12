// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test("dottedAlias", function (): void {
  const argv = parse(["--a.b", "22"], {
    default: { "a.b": 11 },
    alias: { "a.b": "aa.bb" },
  });
  assertEquals(argv.a.b, 22);
  assertEquals(argv.aa.bb, 22);
});

Deno.test("dottedDefault", function (): void {
  const argv = parse([], { default: { "a.b": 11 }, alias: { "a.b": "aa.bb" } });
  assertEquals(argv.a.b, 11);
  assertEquals(argv.aa.bb, 11);
});

Deno.test("dottedDefaultWithNoAlias", function (): void {
  const argv = parse([], { default: { "a.b": 11 } });
  assertEquals(argv.a.b, 11);
});
