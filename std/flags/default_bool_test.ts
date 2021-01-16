// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test("booleanDefaultTrue", function (): void {
  const argv = parse([], {
    boolean: "sometrue",
    default: { sometrue: true },
  });
  assertEquals(argv.sometrue, true);
});

Deno.test("booleanDefaultFalse", function (): void {
  const argv = parse([], {
    boolean: "somefalse",
    default: { somefalse: false },
  });
  assertEquals(argv.somefalse, false);
});

Deno.test("booleanDefaultNull", function (): void {
  const argv = parse([], {
    boolean: "maybe",
    default: { maybe: null },
  });
  assertEquals(argv.maybe, null);
  const argv2 = parse(["--maybe"], {
    boolean: "maybe",
    default: { maybe: null },
  });
  assertEquals(argv2.maybe, true);
});
