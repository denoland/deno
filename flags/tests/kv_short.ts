// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function short() {
  const argv = parse(["-b=123"]);
  assertEq(argv, { b: 123, _: [] });
});

test(function multiShort() {
  const argv = parse(["-a=whatever", "-b=robots"]);
  assertEq(argv, { a: "whatever", b: "robots", _: [] });
});
