// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test("whitespaceShouldBeWhitespace", function (): void {
  assertEquals(parse(["-x", "\t"]).x, "\t");
});
