// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test("hyphen", function (): void {
  assertEquals(parse(["-n", "-"]), { n: "-", _: [] });
  assertEquals(parse(["-"]), { _: ["-"] });
  assertEquals(parse(["-f-"]), { f: "-", _: [] });
  assertEquals(parse(["-b", "-"], { boolean: "b" }), { b: true, _: ["-"] });
  assertEquals(parse(["-s", "-"], { string: "s" }), { s: "-", _: [] });
});

Deno.test("doubleDash", function (): void {
  assertEquals(parse(["-a", "--", "b"]), { a: true, _: ["b"] });
  assertEquals(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
  assertEquals(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
});

Deno.test("moveArgsAfterDoubleDashIntoOwnArray", function (): void {
  assertEquals(
    parse(["--name", "John", "before", "--", "after"], { "--": true }),
    {
      name: "John",
      _: ["before"],
      "--": ["after"],
    },
  );
});
