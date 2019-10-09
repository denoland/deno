// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

test(function short(): void {
  const argv = parse(["-b=123"]);
  assertEquals(argv, { b: 123, _: [] });
});

test(function multiShort(): void {
  const argv = parse(["-a=whatever", "-b=robots"]);
  assertEquals(argv, { a: "whatever", b: "robots", _: [] });
});
