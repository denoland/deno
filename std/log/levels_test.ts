// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { logLevels } from "./levels.ts";

Deno.test("logLevel name", function (): void {
  assertEquals(logLevels.trace.name, "Trace");
  assertEquals(logLevels.debug.name, "Debug");
  assertEquals(logLevels.info.name, "Info");
  assertEquals(logLevels.warn.name, "Warn");
  assertEquals(logLevels.error.name, "Error");
});

Deno.test("logLevel code", function (): void {
  assertEquals(logLevels.trace.code, 10);
  assertEquals(logLevels.debug.code, 20);
  assertEquals(logLevels.info.code, 30);
  assertEquals(logLevels.warn.code, 40);
  assertEquals(logLevels.error.code, 50);
});
