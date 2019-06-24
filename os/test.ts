// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertNotEquals } from "../testing/asserts.ts";
import { userHomeDir } from "./mod.ts";

test(function testUserHomeDir(): void {
  assertNotEquals(userHomeDir(), "");
});
