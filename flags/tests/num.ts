// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function nums() {
  const argv = parse([
    "-x",
    "1234",
    "-y",
    "5.67",
    "-z",
    "1e7",
    "-w",
    "10f",
    "--hex",
    "0xdeadbeef",
    "789"
  ]);
  assertEq(argv, {
    x: 1234,
    y: 5.67,
    z: 1e7,
    w: "10f",
    hex: 0xdeadbeef,
    _: [789]
  });
  assertEq(typeof argv.x, "number");
  assertEq(typeof argv.y, "number");
  assertEq(typeof argv.z, "number");
  assertEq(typeof argv.w, "string");
  assertEq(typeof argv.hex, "number");
  assertEq(typeof argv._[0], "number");
});

test(function alreadyNumber() {
  const argv = parse(["-x", 1234, 789]);
  assertEq(argv, { x: 1234, _: [789] });
  assertEq(typeof argv.x, "number");
  assertEq(typeof argv._[0], "number");
});
