// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

test(function hyphen(): void {
  assertEquals(parse(["-n", "-"]), { n: "-", _: [] });
  assertEquals(parse(["-"]), { _: ["-"] });
  assertEquals(parse(["-f-"]), { f: "-", _: [] });
  assertEquals(parse(["-b", "-"], { boolean: "b" }), { b: true, _: ["-"] });
  assertEquals(parse(["-s", "-"], { string: "s" }), { s: "-", _: [] });
});

test(function doubleDash(): void {
  assertEquals(parse(["-a", "--", "b"]), { a: true, _: ["b"] });
  assertEquals(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
  assertEquals(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
});

test(function moveArgsAfterDoubleDashIntoOwnArray(): void {
  assertEquals(
    parse(["--name", "John", "before", "--", "after"], { "--": true }),
    {
      name: "John",
      _: ["before"],
      "--": ["after"]
    }
  );
});
