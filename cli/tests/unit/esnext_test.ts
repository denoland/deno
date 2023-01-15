// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

// TODO(@kitsonk) remove when we are no longer patching TypeScript to have
// these types available.

Deno.test(function typeCheckingEsNextArrayString() {
  const b = ["a", "b", "c", "d", "e", "f"];
  assertEquals(b.findLast((val) => typeof val === "string"), "f");
  assertEquals(b.findLastIndex((val) => typeof val === "string"), 5);
});

Deno.test(function intlListFormat() {
  const formatter = new Intl.ListFormat("en", {
    style: "long",
    type: "conjunction",
  });
  assertEquals(
    formatter.format(["red", "green", "blue"]),
    "red, green, and blue",
  );

  const formatter2 = new Intl.ListFormat("en", {
    style: "short",
    type: "disjunction",
  });
  assertEquals(formatter2.formatToParts(["Rust", "golang"]), [
    { type: "element", value: "Rust" },
    { type: "literal", value: " or " },
    { type: "element", value: "golang" },
  ]);

  // Works with iterables as well
  assertEquals(
    formatter.format(new Set(["red", "green", "blue"])),
    "red, green, and blue",
  );
  assertEquals(formatter2.formatToParts(new Set(["Rust", "golang"])), [
    { type: "element", value: "Rust" },
    { type: "literal", value: " or " },
    { type: "element", value: "golang" },
  ]);
});
