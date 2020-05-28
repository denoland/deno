// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { Logger } from "./logger.ts";
import { assert } from "../testing/asserts.ts";
import { getLogger } from "./mod.ts";

let logger: Logger | null = null;
try {
  // Need to initialize it here
  // otherwise it will be already initialized on Deno.test
  logger = getLogger();
} catch {}

test("logger is initialized", function (): void {
  assert(logger instanceof Logger);
});
