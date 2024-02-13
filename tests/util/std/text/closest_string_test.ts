// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../assert/mod.ts";
import { closestString } from "./closest_string.ts";

Deno.test("closestString - basic", function () {
  const words = ["hi", "hello", "help"];

  assertEquals(
    JSON.stringify(closestString("hep", words)),
    '"hi"',
  );
});

Deno.test("closestString - caseSensitive1", function () {
  const words = ["hi", "hello", "help"];

  // this is why caseSensitive is OFF by default; very unintuitive until something better than levenshtein_distance is used
  assertEquals(
    JSON.stringify(closestString("HELP", words, { caseSensitive: true })),
    '"hi"',
  );
});

Deno.test("closestString - caseSensitive2", function () {
  const words = ["HI", "HELLO", "HELP"];

  assertEquals(
    JSON.stringify(closestString("he", words, { caseSensitive: true })),
    '"HI"',
  );
});

Deno.test("closestString - empty input", function () {
  assertThrows(
    () => closestString("he", []),
    Error,
    "When using closestString(), the possibleWords array must contain at least one word",
  );
});
