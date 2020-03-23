// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test(function whitespaceShouldBeWhitespace(): void {
  assertEquals(parse(["-x", "\t"]).x, "\t");
});
