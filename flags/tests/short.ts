// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "../../testing/mod.ts";
import { parse } from "../mod.ts";

test(function numbericShortArgs() {
  assertEqual(parse(["-n123"]), { n: 123, _: [] });
  assertEqual(parse(["-123", "456"]), { 1: true, 2: true, 3: 456, _: [] });
});

test(function short() {
  assertEqual(parse(["-b"]), { b: true, _: [] });
  assertEqual(parse(["foo", "bar", "baz"]), { _: ["foo", "bar", "baz"] });
  assertEqual(parse(["-cats"]), { c: true, a: true, t: true, s: true, _: [] });
  assertEqual(parse(["-cats", "meow"]), {
    c: true,
    a: true,
    t: true,
    s: "meow",
    _: []
  });
  assertEqual(parse(["-h", "localhost"]), { h: "localhost", _: [] });
  assertEqual(parse(["-h", "localhost", "-p", "555"]), {
    h: "localhost",
    p: 555,
    _: []
  });
});

test(function mixedShortBoolAndCapture() {
  assertEqual(parse(["-h", "localhost", "-fp", "555", "script.js"]), {
    f: true,
    p: 555,
    h: "localhost",
    _: ["script.js"]
  });
});

test(function shortAndLong() {
  assertEqual(parse(["-h", "localhost", "-fp", "555", "script.js"]), {
    f: true,
    p: 555,
    h: "localhost",
    _: ["script.js"]
  });
});
