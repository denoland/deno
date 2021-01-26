// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test("short", function (): void {
  const argv = parse(["-b=123"]);
  assertEquals(argv, { b: 123, _: [] });
});

Deno.test("multiShort", function (): void {
  const argv = parse(["-a=whatever", "-b=robots"]);
  assertEquals(argv, { a: "whatever", b: "robots", _: [] });
});
