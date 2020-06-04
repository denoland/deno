// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
import { errorf } from './errorf.ts';
import { assertEquals } from "../testing/asserts.ts";

Deno.test("noVerb", function (): void {
  assertEquals(errorf("bla").message, "bla");
});

Deno.test("testString", function (): void {
  assertEquals(errorf("%s", "bla").message, "bla");
});


Deno.test("testBoolean", function (): void {
  assertEquals(errorf("%t", true).message, "true");
  assertEquals(errorf("%t", false).message, "false");
  assertEquals(errorf("bla%t", true).message, "blatrue");
  assertEquals(errorf("%tbla", false).message, "falsebla");
});

