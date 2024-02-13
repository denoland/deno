// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { sort } from "./sort.ts";
import { parse } from "./parse.ts";

Deno.test("sort", function () {
  const list = ["1.2.3+1", "1.2.3+0", "1.2.3", "5.9.6", "0.1.2"];
  const sorted = ["0.1.2", "1.2.3+1", "1.2.3+0", "1.2.3", "5.9.6"];
  assertEquals(sort(list.map((v) => parse(v))), sorted.map((v) => parse(v)));
});
