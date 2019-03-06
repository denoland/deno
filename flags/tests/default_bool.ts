// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function booleanDefaultTrue() {
  const argv = parse([], {
    boolean: "sometrue",
    default: { sometrue: true }
  });
  assertEq(argv.sometrue, true);
});

test(function booleanDefaultFalse() {
  const argv = parse([], {
    boolean: "somefalse",
    default: { somefalse: false }
  });
  assertEq(argv.somefalse, false);
});

test(function booleanDefaultNull() {
  const argv = parse([], {
    boolean: "maybe",
    default: { maybe: null }
  });
  assertEq(argv.maybe, null);
  const argv2 = parse(["--maybe"], {
    boolean: "maybe",
    default: { maybe: null }
  });
  assertEq(argv2.maybe, true);
});
