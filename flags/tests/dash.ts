// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function hyphen() {
  assertEq(parse(["-n", "-"]), { n: "-", _: [] });
  assertEq(parse(["-"]), { _: ["-"] });
  assertEq(parse(["-f-"]), { f: "-", _: [] });
  assertEq(parse(["-b", "-"], { boolean: "b" }), { b: true, _: ["-"] });
  assertEq(parse(["-s", "-"], { string: "s" }), { s: "-", _: [] });
});

test(function doubleDash() {
  assertEq(parse(["-a", "--", "b"]), { a: true, _: ["b"] });
  assertEq(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
  assertEq(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
});

test(function moveArgsAfterDoubleDashIntoOwnArray() {
  assertEq(parse(["--name", "John", "before", "--", "after"], { "--": true }), {
    name: "John",
    _: ["before"],
    "--": ["after"]
  });
});
