// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function numbericShortArgs() {
  assertEq(parse(["-n123"]), { n: 123, _: [] });
  assertEq(parse(["-123", "456"]), { 1: true, 2: true, 3: 456, _: [] });
});

test(function short() {
  assertEq(parse(["-b"]), { b: true, _: [] });
  assertEq(parse(["foo", "bar", "baz"]), { _: ["foo", "bar", "baz"] });
  assertEq(parse(["-cats"]), { c: true, a: true, t: true, s: true, _: [] });
  assertEq(parse(["-cats", "meow"]), {
    c: true,
    a: true,
    t: true,
    s: "meow",
    _: []
  });
  assertEq(parse(["-h", "localhost"]), { h: "localhost", _: [] });
  assertEq(parse(["-h", "localhost", "-p", "555"]), {
    h: "localhost",
    p: 555,
    _: []
  });
});

test(function mixedShortBoolAndCapture() {
  assertEq(parse(["-h", "localhost", "-fp", "555", "script.js"]), {
    f: true,
    p: 555,
    h: "localhost",
    _: ["script.js"]
  });
});

test(function shortAndLong() {
  assertEq(parse(["-h", "localhost", "-fp", "555", "script.js"]), {
    f: true,
    p: 555,
    h: "localhost",
    _: ["script.js"]
  });
});
