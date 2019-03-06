// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

// flag boolean true (default all --args to boolean)
test(function flagBooleanTrue() {
  const argv = parse(["moo", "--honk", "cow"], {
    boolean: true
  });

  assertEq(argv, {
    honk: true,
    _: ["moo", "cow"]
  });

  assertEq(typeof argv.honk, "boolean");
});

// flag boolean true only affects double hyphen arguments without equals signs
test(function flagBooleanTrueOnlyAffectsDoubleDash() {
  var argv = parse(["moo", "--honk", "cow", "-p", "55", "--tacos=good"], {
    boolean: true
  });

  assertEq(argv, {
    honk: true,
    tacos: "good",
    p: 55,
    _: ["moo", "cow"]
  });

  assertEq(typeof argv.honk, "boolean");
});
