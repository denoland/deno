// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

test(function dottedAlias() {
  const argv = parse(["--a.b", "22"], {
    default: { "a.b": 11 },
    alias: { "a.b": "aa.bb" }
  });
  assertEquals(argv.a.b, 22);
  assertEquals(argv.aa.bb, 22);
});

test(function dottedDefault() {
  const argv = parse("", { default: { "a.b": 11 }, alias: { "a.b": "aa.bb" } });
  assertEquals(argv.a.b, 11);
  assertEquals(argv.aa.bb, 11);
});

test(function dottedDefaultWithNoAlias() {
  const argv = parse("", { default: { "a.b": 11 } });
  assertEquals(argv.a.b, 11);
});
