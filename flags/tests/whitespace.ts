// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function whitespaceShouldBeWhitespace() {
  assertEq(parse(["-x", "\t"]).x, "\t");
});
