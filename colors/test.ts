// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { red, bgBlue, setEnabled, getEnabled } from "./mod.ts";
import "./example.ts";

test(function singleColor() {
  assertEq(red("Hello world"), "[31mHello world[39m");
});

test(function doubleColor() {
  assertEq(bgBlue(red("Hello world")), "[44m[31mHello world[39m[49m");
});

test(function replacesCloseCharacters() {
  assertEq(red("Hel[39mlo"), "[31mHel[31mlo[39m");
});

test(function enablingColors() {
  assertEq(getEnabled(), true);
  setEnabled(false);
  assertEq(bgBlue(red("Hello world")), "Hello world");
  setEnabled(true);
  assertEq(red("Hello world"), "[31mHello world[39m");
});
