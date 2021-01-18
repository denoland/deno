// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

// flag boolean true (default all --args to boolean)
Deno.test("flagBooleanTrue", function (): void {
  const argv = parse(["moo", "--honk", "cow"], {
    boolean: true,
  });

  assertEquals(argv, {
    honk: true,
    _: ["moo", "cow"],
  });

  assertEquals(typeof argv.honk, "boolean");
});

// flag boolean true only affects double hyphen arguments without equals signs
Deno.test("flagBooleanTrueOnlyAffectsDoubleDash", function (): void {
  const argv = parse(["moo", "--honk", "cow", "-p", "55", "--tacos=good"], {
    boolean: true,
  });

  assertEquals(argv, {
    honk: true,
    tacos: "good",
    p: 55,
    _: ["moo", "cow"],
  });

  assertEquals(typeof argv.honk, "boolean");
});
